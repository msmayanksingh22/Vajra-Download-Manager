use std::process::Stdio;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::watch,
};

use crate::download_task::{ControlSignal, DownloadProgress, DownloadRequest, TaskId, TaskState};

pub async fn download_ytdlp(
    _id: TaskId,
    req: &DownloadRequest,
    tx: &watch::Sender<DownloadProgress>,
    ctrl: &mut tokio::sync::oneshot::Receiver<ControlSignal>,
) -> anyhow::Result<u64> {
    let mut p = tx.borrow().clone();
    p.state = TaskState::Downloading;
    let _ = tx.send(p.clone());

    let out_dir = &req.dest_dir;
    let filename = req
        .filename
        .clone()
        .unwrap_or_else(|| "%(title)s.%(ext)s".to_string());
    let mut out_path = out_dir.clone();
    out_path.push(filename);

    let ytdlp_path = if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            #[cfg(target_os = "windows")]
            let local = parent.join("yt-dlp.exe");
            #[cfg(not(target_os = "windows"))]
            let local = parent.join("yt-dlp");

            if local.exists() {
                local.to_string_lossy().to_string()
            } else {
                "yt-dlp".to_string()
            }
        } else {
            "yt-dlp".to_string()
        }
    } else {
        "yt-dlp".to_string()
    };

    let mut cmd = Command::new(ytdlp_path);
    cmd.arg("--no-update");
    cmd.arg("--newline");
    cmd.arg("--progress-template");
    cmd.arg("%(progress._percent_str)s|%(progress._total_bytes_str)s|%(progress._speed_str)s|%(progress._eta_str)s");

    // Pass headers
    if let Some(cookie) = &req.cookie_header {
        cmd.arg("--add-header").arg(format!("Cookie: {}", cookie));
    }
    if let Some(referrer) = &req.referrer {
        cmd.arg("--add-header")
            .arg(format!("Referer: {}", referrer));
    }
    if let Some(ua) = &req.user_agent {
        cmd.arg("--user-agent").arg(ua);
    }
    if let Some(proxy) = &req.proxy {
        cmd.arg("--proxy").arg(proxy);
    }
    if let Some(auth) = &req.authorization {
        cmd.arg("--add-header")
            .arg(format!("Authorization: {}", auth));
    }

    if let Some(fmt) = &req.ytdlp_format {
        if !fmt.is_empty() {
            cmd.arg("-f").arg(fmt);
        }
    }
    if req.ytdlp_subtitles {
        cmd.arg("--write-subs").arg("--write-auto-subs").arg("--embed-subs");
    }
    if req.ytdlp_playlist {
        cmd.arg("--yes-playlist");
    } else {
        cmd.arg("--no-playlist");
    }

    cmd.arg("-o").arg(out_path.to_string_lossy().as_ref());
    cmd.arg(&req.url);

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Also redirect stdin to null so yt-dlp doesn't try to prompt
    cmd.stdin(Stdio::null());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to capture yt-dlp stdout"))?;
    let mut reader = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            _ = &mut *ctrl => {
                let _ = child.kill().await;
                anyhow::bail!("Cancelled");
            }
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        // parse line "0.1%|967.79KiB|Unknown B/s|Unknown"
                        let parts: Vec<&str> = line.split('|').collect();
                        if parts.len() == 4 {
                            let percent_str = parts[0].trim().trim_end_matches('%');
                            if let Ok(pct) = percent_str.parse::<f64>() {
                                p.progress_fraction = pct / 100.0;
                            }
                            if let Some(bytes) = parse_bytes(parts[1].trim()) {
                                p.total_bytes = bytes;
                                p.bytes_downloaded = (p.total_bytes as f64 * p.progress_fraction) as u64;
                            }
                            if let Some(speed) = parse_bytes(parts[2].trim().trim_end_matches("/s")) {
                                p.speed_bps = speed;
                            }
                            if let Some(eta) = parse_time(parts[3].trim()) {
                                p.eta_secs = eta;
                            }
                            let _ = tx.send(p.clone());
                        } else if line.starts_with("[download] Destination:") {
                            let dest = line.trim_start_matches("[download] Destination:").trim();
                            p.dest_path = dest.to_string();
                            p.filename = std::path::Path::new(dest).file_name().unwrap_or_default().to_string_lossy().into_owned();
                            let _ = tx.send(p.clone());
                        } else if line.starts_with("[download]") && line.contains("has already been downloaded") {
                            p.progress_fraction = 1.0;
                            let _ = tx.send(p.clone());
                        }
                    }
                    Ok(None) => break,
                    Err(e) => anyhow::bail!("Stdout error: {}", e),
                }
            }
        }
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("yt-dlp exited with status: {}", status);
    }

    if !p.dest_path.is_empty() {
        if let Ok(meta) = std::fs::metadata(&p.dest_path) {
            return Ok(meta.len());
        }
    }

    Ok(p.total_bytes)
}

fn parse_bytes(s: &str) -> Option<u64> {
    if s == "Unknown" || s.contains("NA") {
        return None;
    }
    let s = s.to_uppercase();
    let mut num_str = s.clone();
    let mut multiplier: f64 = 1.0;

    if s.ends_with("KIB") {
        multiplier = 1024.0;
        num_str = s.trim_end_matches("KIB").to_string();
    } else if s.ends_with("MIB") {
        multiplier = 1024.0 * 1024.0;
        num_str = s.trim_end_matches("MIB").to_string();
    } else if s.ends_with("GIB") {
        multiplier = 1024.0 * 1024.0 * 1024.0;
        num_str = s.trim_end_matches("GIB").to_string();
    } else if s.ends_with("KB") {
        multiplier = 1000.0;
        num_str = s.trim_end_matches("KB").to_string();
    } else if s.ends_with("MB") {
        multiplier = 1000000.0;
        num_str = s.trim_end_matches("MB").to_string();
    } else if s.ends_with("GB") {
        multiplier = 1000000000.0;
        num_str = s.trim_end_matches("GB").to_string();
    } else if s.ends_with("B") {
        num_str = s.trim_end_matches("B").to_string();
    }

    num_str
        .trim()
        .parse::<f64>()
        .ok()
        .map(|n| (n * multiplier) as u64)
}

fn parse_time(s: &str) -> Option<u64> {
    if s == "Unknown" || s.contains("NA") {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let m = parts[0].parse::<u64>().ok()?;
        let sec = parts[1].parse::<u64>().ok()?;
        Some(m * 60 + sec)
    } else if parts.len() == 3 {
        let h = parts[0].parse::<u64>().ok()?;
        let m = parts[1].parse::<u64>().ok()?;
        let sec = parts[2].parse::<u64>().ok()?;
        Some(h * 3600 + m * 60 + sec)
    } else {
        None
    }
}
