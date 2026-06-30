use std::{io::Write, sync::Arc};

use anyhow::{Context, Result};
use reqwest::{Client, Url};
use tokio::sync::{watch, Mutex};

use crate::{
    download_task::{ControlSignal, DownloadProgress, DownloadRequest, TaskId, TaskState},
    ffmpeg,
};

#[derive(Debug, Clone)]
pub struct HlsSegment {
    pub url: String,
    pub duration: f32,
}

#[derive(Debug, Clone)]
pub struct HlsPlaylist {
    pub segments: Vec<HlsSegment>,
    pub target_duration: f32,
}

pub fn parse_m3u8(base_url: &str, content: &str) -> Result<HlsPlaylist> {
    let base = Url::parse(base_url).context("Invalid base URL")?;
    let mut segments = Vec::new();
    let mut target_duration = 0.0;
    let mut current_duration = 0.0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXT-X-TARGETDURATION:") {
            if let Some(dur) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
                target_duration = dur.parse().unwrap_or(0.0);
            }
        } else if line.starts_with("#EXTINF:") {
            if let Some(info) = line.strip_prefix("#EXTINF:") {
                let dur_str = info.split(',').next().unwrap_or("0");
                current_duration = dur_str.parse().unwrap_or(0.0);
            }
        } else if !line.starts_with('#') {
            let segment_url = base.join(line).context("Failed to join segment URL")?;
            segments.push(HlsSegment {
                url: segment_url.to_string(),
                duration: current_duration,
            });
            current_duration = 0.0;
        }
    }

    Ok(HlsPlaylist {
        segments,
        target_duration,
    })
}

pub async fn download_hls(
    id: TaskId,
    req: &DownloadRequest,
    tx: &watch::Sender<DownloadProgress>,
    ctrl: &mut tokio::sync::oneshot::Receiver<ControlSignal>,
) -> Result<u64> {
    emit(tx, id, |p| p.state = TaskState::FetchingMeta);

    let client = Client::builder()
        .user_agent(req.user_agent.as_deref().unwrap_or("Vajra/1.0"))
        .build()?;

    let m3u8_resp = client.get(&req.url).send().await?.text().await?;
    let playlist = parse_m3u8(&req.url, &m3u8_resp)?;

    if playlist.segments.is_empty() {
        anyhow::bail!("No segments found in m3u8 playlist");
    }

    emit(tx, id, |p| p.state = TaskState::Allocating);

    let temp_dir = req.dest_dir.join(format!(".hls_temp_{}", id));
    std::fs::create_dir_all(&temp_dir)?;

    let total_segments = playlist.segments.len();
    let completed_segments = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    emit(tx, id, |p| {
        p.state = TaskState::Downloading;
        p.total_bytes = total_segments as u64; // Using segments as progress proxy
    });

    // Create a queue of segments
    let mut tasks = Vec::new();
    let queue = Arc::new(Mutex::new(
        playlist
            .segments
            .into_iter()
            .enumerate()
            .collect::<Vec<_>>(),
    ));

    for _ in 0..req.max_connections {
        let q = queue.clone();
        let client_clone = client.clone();
        let temp_dir_clone = temp_dir.clone();
        let completed = completed_segments.clone();
        let tx_clone = tx.clone();

        let handle = tokio::spawn(async move {
            loop {
                let segment = {
                    let mut lock = q.lock().await;
                    match lock.pop() {
                        Some(s) => s,
                        None => break,
                    }
                };

                let (idx, seg) = segment;
                let ts_path = temp_dir_clone.join(format!("{:05}.ts", idx));

                if !ts_path.exists() {
                    let resp = client_clone.get(&seg.url).send().await;
                    match resp {
                        Ok(mut response) => {
                            if !response.status().is_success() {
                                anyhow::bail!("HTTP {} for segment {}", response.status(), seg.url);
                            }
                            match std::fs::File::create(&ts_path) {
                                Ok(mut file) => {
                                    while let Ok(Some(chunk)) = response.chunk().await {
                                        if let Err(e) = file.write_all(&chunk) {
                                            let _ = std::fs::remove_file(&ts_path);
                                            anyhow::bail!(
                                                "Failed to write segment {}: {}",
                                                seg.url,
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    anyhow::bail!(
                                        "Failed to create HLS segment file {:?}: {}",
                                        ts_path,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            anyhow::bail!("Failed to download segment {}: {}", seg.url, e);
                        }
                    }
                }

                let count = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                emit(&tx_clone, id, |p| {
                    p.bytes_downloaded = count as u64;
                    p.progress_fraction = (count as f64) / (total_segments as f64);
                });
            }
            Ok::<(), anyhow::Error>(())
        });
        tasks.push(handle);
    }

    // Wait for all to complete or cancellation
    tokio::select! {
        results = futures::future::join_all(&mut tasks) => {
            for res in results {
                res??; // Unpack JoinError and then our anyhow::Error
            }
        }
        signal = ctrl => {
            for t in tasks {
                t.abort();
            }
            match signal {
                Ok(ControlSignal::Cancel) => {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Err(crate::download_task::DownloadError::Cancelled.into());
                }
                Ok(ControlSignal::Pause) => {
                    // HLS supports resume mid-download because of the `!ts_path.exists()` check.
                    // Do NOT clean up partial temp files so we can resume later!
                    return Err(crate::download_task::DownloadError::Paused.into());
                }
                Err(_) => {} // sender dropped — continue
            }
        }
    }

    emit(tx, id, |p| p.state = TaskState::Verifying); // Or Muxing state, reusing Verifying for now

    // Muxing
    let concat_path = temp_dir.join("concat.txt");
    let mut concat_file = std::fs::File::create(&concat_path)?;
    for i in 0..total_segments {
        let ts_name = format!("{:05}.ts", i);
        writeln!(concat_file, "file '{}'", ts_name)?;
    }

    let mut dest_path = req.dest_dir.clone();
    let filename = req
        .filename
        .clone()
        .unwrap_or_else(|| "hls_download.mp4".to_string());
    dest_path.push(filename);

    ffmpeg::mux_ts_files(&concat_path, &dest_path).await?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);

    emit(tx, id, |p| p.state = TaskState::Completed);

    Ok(total_segments as u64)
}

fn emit<F: FnOnce(&mut DownloadProgress)>(tx: &watch::Sender<DownloadProgress>, _id: TaskId, f: F) {
    tx.send_modify(f);
}
