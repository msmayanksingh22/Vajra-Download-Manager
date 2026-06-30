use std::{path::PathBuf, sync::Arc, time::Duration};

use librqbit::{AddTorrent, Session, SessionOptions};
use tokio::sync::{oneshot, watch, OnceCell};

use crate::download_task::{ControlSignal, DownloadProgress, TaskId, TaskState};

static GLOBAL_SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

fn extract_torrent_name(url: &str) -> String {
    if url.starts_with("magnet:") {
        if let Some(query) = url.split_once('?').map(|x| x.1) {
            // First pass: look for 'dn' (display name)
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "dn" {
                        if let Ok(decoded) = urlencoding::decode(v) {
                            return decoded.into_owned().replace('+', " ");
                        }
                    }
                }
            }
            // Second pass: look for 'xt' to use hash
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "xt" && v.starts_with("urn:btih:") {
                        let hash = v.trim_start_matches("urn:btih:");
                        return format!("Torrent_{}", hash);
                    }
                }
            }
        }
        "Vajra_Torrent".to_string()
    } else {
        if let Some(last_slash) = url.split('/').next_back() {
            let file_name = last_slash.split('?').next().unwrap_or("Vajra_Torrent");
            if file_name.ends_with(".torrent") {
                return file_name.trim_end_matches(".torrent").to_string();
            }
            return file_name.to_string();
        }
        "Vajra_Torrent".to_string()
    }
}

pub async fn start_torrent(
    _id: TaskId,
    url: String,
    dest_dir: PathBuf,
    filename: Option<String>,
    progress_tx: watch::Sender<DownloadProgress>,
    mut ctrl_rx: oneshot::Receiver<ControlSignal>,
) -> anyhow::Result<u64> {
    // Get or initialize the global librqbit session.
    let session = GLOBAL_SESSION
        .get_or_try_init(|| async {
            // We use a generic default output folder, but we override it per torrent below.
            let default_dir = dest_dir.clone();
            let opts = SessionOptions {
                listen_port_range: Some(6881..6890), // Allow some port flexibility
                disable_dht_persistence: true,       // Prevent port-binding collisions across restarts
                ..Default::default()
            };
            Session::new_with_opts(default_dir, opts).await
        })
        .await?;

    // Add the torrent (either magnet or local .torrent file path/URL)
    let add_torrent = AddTorrent::from_url(url.clone());

    let torrent_name = if let Some(ref f) = filename {
        if !f.is_empty() {
            f.clone()
        } else {
            extract_torrent_name(&url)
        }
    } else {
        extract_torrent_name(&url)
    };
    let final_dest_dir = dest_dir.join(torrent_name);
    let _ = std::fs::create_dir_all(&final_dest_dir);

    // We override the output folder for THIS specific torrent, since the session is global.
    let add_opts = librqbit::AddTorrentOptions {
        output_folder: Some(final_dest_dir.to_string_lossy().into_owned()),
        overwrite: true, // Required to not fail immediately if target exists
        paused: false,   // Always start unpaused since DownloadTask::start implies we want it running
        ..Default::default()
    };

    let handle = session
        .add_torrent(add_torrent, Some(add_opts))
        .await?
        .into_handle()
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire torrent handle"))?;

    // If the torrent was already in the session and paused, we must unpause it
    let _ = session.unpause(&handle).await;

    let mut filename = String::new();
    let mut dest_path = String::new();

    // Polling loop
    loop {
        // Handle control signals
        if let Ok(signal) = ctrl_rx.try_recv() {
            match signal {
                ControlSignal::Pause => {
                    let _ = session
                        .delete(
                            librqbit::api::TorrentIdOrHash::Hash(handle.info_hash()),
                            false,
                        )
                        .await;
                    progress_tx.send_modify(|state| {
                        state.state = TaskState::Paused;
                    });
                    return Err(anyhow::anyhow!("Paused"));
                }
                ControlSignal::Cancel => {
                    let _ = session
                        .delete(
                            librqbit::api::TorrentIdOrHash::Hash(handle.info_hash()),
                            true,
                        )
                        .await;
                    progress_tx.send_modify(|state| {
                        state.state = TaskState::Cancelled;
                    });
                    return Err(anyhow::anyhow!("Cancelled"));
                }
            }
        }

        let stats = handle.stats();
        let mut speed_bps = 0;
        let mut eta_secs = 0;

        if let Some(live) = &stats.live {
            speed_bps = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
        }
        if speed_bps > 0 && stats.progress_bytes < stats.total_bytes {
            eta_secs = ((stats.total_bytes - stats.progress_bytes) / speed_bps) as u64;
        }

        if filename.is_empty() {
            if let Some(name) = handle.name() {
                filename = name.to_string();
            } else {
                let hash = handle.info_hash().as_string();
                filename = hash.clone();
            }
            // Always set dest_path to the folder we created for this torrent
            dest_path = final_dest_dir.to_string_lossy().into_owned();
        }

        let total_bytes = stats.total_bytes;
        let bytes_downloaded = stats.progress_bytes;
        let mut progress_fraction = 0.0;
        if total_bytes > 0 {
            progress_fraction = bytes_downloaded as f64 / total_bytes as f64;
        }

        let is_completed = bytes_downloaded >= total_bytes && total_bytes > 0;
        let task_state = if is_completed {
            TaskState::Completed
        } else {
            match stats.state {
                librqbit::TorrentStatsState::Paused => TaskState::Paused,
                librqbit::TorrentStatsState::Error => TaskState::Failed,
                _ => TaskState::Downloading,
            }
        };

        progress_tx.send_modify(|s| {
            s.total_bytes = total_bytes;
            s.bytes_downloaded = bytes_downloaded;
            s.speed_bps = speed_bps;
            s.eta_secs = eta_secs;
            s.progress_fraction = progress_fraction;
            s.state = task_state.clone();

            if !filename.is_empty() && s.filename != filename {
                s.filename = filename.clone();
            }

            // Always ensure the dest_path points to the target folder or file accurately
            // For torrents, final_dest_dir contains all the downloaded data.
            if !dest_path.is_empty() {
                s.dest_path = dest_path.clone();
            } else {
                s.dest_path = final_dest_dir.to_string_lossy().into_owned();
            }
        });

        if is_completed {
            let _ = session.delete(librqbit::api::TorrentIdOrHash::Hash(handle.info_hash()), false).await;
            break;
        }

        if matches!(task_state, TaskState::Failed) {
            let _ = session.delete(librqbit::api::TorrentIdOrHash::Hash(handle.info_hash()), false).await;
            return Err(anyhow::anyhow!("Torrent failed"));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let final_bytes = handle.stats().progress_bytes;
    Ok(final_bytes)
}
