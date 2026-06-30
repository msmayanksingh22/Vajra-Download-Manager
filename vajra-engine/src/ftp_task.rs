use std::time::{Duration, Instant};

use futures::io::AsyncReadExt as _;
use suppaftp::AsyncFtpStream;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::watch,
};
use url::Url;

use crate::download_task::{ControlSignal, DownloadProgress, DownloadRequest, TaskId, TaskState};

pub async fn download_ftp(
    _id: TaskId,
    req: &DownloadRequest,
    tx: &watch::Sender<DownloadProgress>,
    ctrl: &mut tokio::sync::oneshot::Receiver<ControlSignal>,
) -> anyhow::Result<u64> {
    let mut p = tx.borrow().clone();
    p.state = TaskState::Allocating;
    let _ = tx.send(p.clone());

    let parsed_url = Url::parse(&req.url).map_err(|e| anyhow::anyhow!("Invalid FTP URL: {}", e))?;
    let host = parsed_url.host_str().unwrap_or("localhost");
    let port = parsed_url.port().unwrap_or(21);
    let username = if parsed_url.username().is_empty() {
        "anonymous"
    } else {
        parsed_url.username()
    };
    let password = parsed_url.password().unwrap_or("anonymous@example.com");
    let path = parsed_url.path();

    // Setup destination file
    let mut dest_path = req.dest_dir.clone();
    let file_name = req.filename.clone().unwrap_or_else(|| {
        path.split('/')
            .next_back()
            .unwrap_or("download.ftp")
            .to_string()
    });
    dest_path.push(&file_name);

    p.filename = file_name.clone();
    p.dest_path = dest_path.to_string_lossy().to_string();
    let _ = tx.send(p.clone());

    // Connect to FTP server
    let mut ftp_stream = AsyncFtpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(|e| anyhow::anyhow!("FTP Connect Error: {}", e))?;

    ftp_stream
        .login(username, password)
        .await
        .map_err(|e| anyhow::anyhow!("FTP Login Error: {}", e))?;

    // Get file size
    let size = ftp_stream
        .size(path)
        .await
        .map_err(|e| anyhow::anyhow!("FTP Size Error (maybe unsupported?): {}", e))?
        as u64;

    p.total_bytes = size;
    let _ = tx.send(p.clone());

    // Determine starting offset
    let mut start_offset = 0u64;
    if dest_path.exists() {
        if let Ok(meta) = fs::metadata(&dest_path).await {
            start_offset = meta.len();
            if start_offset > size && size > 0 {
                // If the file on disk is larger than the remote, start over
                start_offset = 0;
            } else if start_offset == size && size > 0 {
                // Already complete
                p.bytes_downloaded = size;
                p.progress_fraction = 1.0;
                let _ = tx.send(p.clone());
                return Ok(size);
            }
        }
    }

    if start_offset > 0 {
        p.bytes_downloaded = start_offset;
        p.progress_fraction = start_offset as f64 / size.max(1) as f64;
        let _ = tx.send(p.clone());
        ftp_stream
            .resume_transfer(start_offset as usize)
            .await
            .map_err(|e| anyhow::anyhow!("FTP Resume Error: {}", e))?;
    }

    // Open file for appending or creating
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(start_offset > 0)
        .truncate(start_offset == 0)
        .open(&dest_path)
        .await?;

    p.state = TaskState::Downloading;
    let _ = tx.send(p.clone());

    // Start data transfer
    let mut data_stream = ftp_stream
        .retr_as_stream(path)
        .await
        .map_err(|e| anyhow::anyhow!("FTP RETR Error: {}", e))?;

    let mut downloaded = start_offset;
    let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer

    let mut last_update = Instant::now();
    let mut last_downloaded = downloaded;

    loop {
        // Check for control signals
        if let Ok(signal) = ctrl.try_recv() {
            match signal {
                ControlSignal::Pause => {
                    p.state = TaskState::Pausing;
                    let _ = tx.send(p.clone());
                    // Clean exit, we can resume later
                    return Ok(downloaded);
                }
                ControlSignal::Cancel => {
                    p.state = TaskState::Cancelled;
                    let _ = tx.send(p.clone());
                    // Try to remove incomplete file if configured
                    if req.delete_on_failure {
                        let _ = fs::remove_file(&dest_path).await;
                    }
                    anyhow::bail!("Download cancelled by user");
                }
            }
        }

        let n = match data_stream.read(&mut buffer).await {
            Ok(0) => break, // EOF
            Ok(n) => n,
            Err(e) => anyhow::bail!("Error reading from FTP stream: {}", e),
        };

        if let Err(e) = file.write_all(&buffer[..n]).await {
            anyhow::bail!("Error writing to file: {}", e);
        }

        downloaded += n as u64;

        // Apply throttle if exists
        if let Some(throttle) = &req.throttle {
            throttle.acquire(n as u64).await;
        }

        let now = Instant::now();
        if now.duration_since(last_update) >= Duration::from_millis(500) {
            let elapsed = now.duration_since(last_update).as_secs_f64();
            let bytes_since = downloaded - last_downloaded;
            let speed_bps = (bytes_since as f64 / elapsed) as u64;

            p.bytes_downloaded = downloaded;
            p.speed_bps = speed_bps;
            if size > 0 {
                p.progress_fraction = downloaded as f64 / size as f64;
                let remaining = size.saturating_sub(downloaded);
                p.eta_secs = remaining.checked_div(speed_bps).unwrap_or(0);
            }
            let _ = tx.send(p.clone());

            last_update = now;
            last_downloaded = downloaded;
        }
    }

    // Flush file
    file.flush().await?;

    // Close FTP connection cleanly
    let _ = ftp_stream.finalize_retr_stream(data_stream).await;
    let _ = ftp_stream.quit().await;

    // Final update
    p.bytes_downloaded = downloaded;
    p.progress_fraction = 1.0;
    p.speed_bps = 0;
    p.eta_secs = 0;
    let _ = tx.send(p.clone());

    Ok(downloaded)
}
