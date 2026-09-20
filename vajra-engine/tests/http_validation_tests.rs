use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;
use vajra_engine::download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskState};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ─── Flexible Mock HTTP Server for Response Validation ────────────────────────

#[derive(Clone)]
enum ServerMode {
    /// Normal server responding with 206 to Range requests and 200 to non-range requests
    Normal,
    /// Responds with HTTP 200 OK (full body) to requests that sent a Range header
    RangedReturns200,
    /// Responds with HTTP 416 Range Not Satisfiable
    Return416,
    /// Responds with 206 Partial Content but abruptly closes socket after sending only `bytes_to_send`
    TruncatedBody { bytes_to_send: usize },
    /// Responds with 200 OK without Content-Length and abruptly closes socket after sending only `bytes_to_send`
    CloseAfter { bytes_to_send: usize },
    /// Non-ranged server: does NOT advertise Accept-Ranges in HEAD, serves 200 OK to GET
    NonRanged,
}

struct MockServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
}

impl MockServer {
    async fn start(data: Vec<u8>, mode: ServerMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let data = Arc::new(data);

        tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let data = Arc::clone(&data);
                let mode = mode.clone();

                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req_str.lines().next().unwrap_or("");
                    let is_head = first_line.starts_with("HEAD");
                    let total_len = data.len();

                    let mut range: Option<(usize, usize)> = None;
                    for line in req_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("range:") {
                            if let Some(bytes_part) = line.split('=').nth(1) {
                                let parts: Vec<&str> = bytes_part.trim().split('-').collect();
                                if let Ok(start) = parts[0].parse::<usize>() {
                                    let end = if parts.len() > 1 && !parts[1].is_empty() {
                                        parts[1].parse::<usize>().unwrap_or(total_len - 1)
                                    } else {
                                        total_len - 1
                                    };
                                    range = Some((start, end.min(total_len - 1)));
                                }
                            }
                        }
                    }

                    match mode {
                        ServerMode::Normal => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else if let Some((start, end)) = range {
                                let chunk_len = end.saturating_sub(start) + 1;
                                let resp = format!(
                                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    start, end, total_len, chunk_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&data[start..=end]).await;
                            } else {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&data).await;
                            }
                        }
                        ServerMode::RangedReturns200 => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else {
                                // Ignore Range and return HTTP 200 OK with full data from byte 0
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&data).await;
                            }
                        }
                        ServerMode::Return416 => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else {
                                let resp = format!(
                                    "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            }
                        }
                        ServerMode::TruncatedBody { bytes_to_send } => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else if let Some((start, end)) = range {
                                let chunk_len = end.saturating_sub(start) + 1;
                                let resp = format!(
                                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    start, end, total_len, chunk_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let to_write = bytes_to_send.min(chunk_len);
                                let _ = socket.write_all(&data[start..start + to_write]).await;
                                // Force close connection abruptly
                                let _ = socket.shutdown().await;
                            }
                        }
                        ServerMode::CloseAfter { bytes_to_send } => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else {
                                let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n";
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let to_write = bytes_to_send.min(total_len);
                                let _ = socket.write_all(&data[..to_write]).await;
                                let _ = socket.shutdown().await;
                            }
                        }
                        ServerMode::NonRanged => {
                            if is_head {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: none\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&data).await;
                            }
                        }
                    }
                });
            }
        });

        Self { addr, shutdown }
    }

    fn url(&self) -> String {
        format!("http://{}/testfile.bin", self.addr)
    }

    fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn generate_deterministic_payload(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state: u64 = 0x12345678_9abcdef0;
    for _ in 0..size {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        data.push((state >> 32) as u8);
    }
    data
}

fn make_request(
    url: &str,
    dest_dir: PathBuf,
    filename: &str,
    max_connections: u32,
) -> DownloadRequest {
    DownloadRequest {
        url: url.to_string(),
        dest_dir,
        filename: Some(filename.to_string()),
        max_connections,
        speed_limit: 0,
        delete_on_failure: false,
        ..Default::default()
    }
}

async fn wait_for_terminal_state(task: &DownloadTask, timeout: Duration) -> DownloadProgress {
    let start = Instant::now();
    let mut rx = task.progress_rx.clone();
    loop {
        let p = rx.borrow().clone();
        if p.state == TaskState::Completed
            || p.state == TaskState::Failed
            || p.state == TaskState::Cancelled
        {
            return p;
        }
        if start.elapsed() > timeout {
            panic!(
                "Timed out waiting for terminal state; current state={:?}",
                p.state
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }
}

async fn wait_for_state(
    task: &DownloadTask,
    target: TaskState,
    timeout: Duration,
) -> DownloadProgress {
    let start = Instant::now();
    let mut rx = task.progress_rx.clone();
    loop {
        let p = rx.borrow().clone();
        if p.state == target {
            return p;
        }
        if start.elapsed() > timeout {
            panic!(
                "Timed out waiting for state {:?}; current state={:?}",
                target, p.state
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }
}

fn verify_file_sha256(path: &std::path::Path, expected: &[u8]) {
    let actual_bytes = std::fs::read(path).expect("failed to read output file");
    let mut hasher = Sha256::new();
    hasher.update(&actual_bytes);
    let actual_hash = hex::encode(hasher.finalize());

    let mut expected_hasher = Sha256::new();
    expected_hasher.update(expected);
    let expected_hash = hex::encode(expected_hasher.finalize());

    assert_eq!(
        actual_hash, expected_hash,
        "SHA-256 mismatch on downloaded file"
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// a) Ranged request returns 200 OK instead of 206 Partial Content:
/// Must be rejected and NOT marked Completed.
#[tokio::test]
async fn test_ranged_request_receiving_200_is_rejected() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 1024 * 1024; // 1 MB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data, ServerMode::RangedReturns200).await;

    let filename = "test_ranged_200.bin";
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 2);

    let task = DownloadTask::start_with_id(task_id, req);
    let terminal = wait_for_terminal_state(&task, Duration::from_secs(5)).await;

    assert_eq!(
        terminal.state,
        TaskState::Failed,
        "Download must fail when server sends HTTP 200 to a ranged request"
    );
    let err_msg = terminal.error.unwrap_or_default();
    assert!(
        err_msg.contains("200") || err_msg.contains("206") || err_msg.contains("status"),
        "Error message should explain HTTP status violation: {}",
        err_msg
    );

    server.stop();
}

/// b) Ranged request returns 416 Range Not Satisfiable:
/// Must be rejected and NOT marked Completed.
#[tokio::test]
async fn test_ranged_request_receiving_416_is_rejected() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 1024 * 1024; // 1 MB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data, ServerMode::Return416).await;

    let filename = "test_ranged_416.bin";
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 2);

    let task = DownloadTask::start_with_id(task_id, req);
    let terminal = wait_for_terminal_state(&task, Duration::from_secs(5)).await;

    assert_eq!(
        terminal.state,
        TaskState::Failed,
        "Download must fail when server returns HTTP 416"
    );
    let err_msg = terminal.error.unwrap_or_default();
    assert!(
        err_msg.contains("416") || err_msg.contains("Range Not Satisfiable"),
        "Error message should mention 416 or Range Not Satisfiable: {}",
        err_msg
    );

    server.stop();
}

/// c) Response ends before expected bytes (Premature EOF):
/// Must detect premature EOF, fail after retries, and NOT mark Completed.
#[tokio::test]
async fn test_early_eof_truncated_response_is_detected() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 512 * 1024; // 512 KB
    let source_data = generate_deterministic_payload(total_size);
    // Server truncates body after sending only 16 KB
    let server = MockServer::start(
        source_data,
        ServerMode::TruncatedBody {
            bytes_to_send: 16 * 1024,
        },
    )
    .await;

    let filename = "test_truncated.bin";
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 1);

    let task = DownloadTask::start_with_id(task_id, req);
    let terminal = wait_for_terminal_state(&task, Duration::from_secs(8)).await;

    assert_eq!(
        terminal.state,
        TaskState::Failed,
        "Download must fail when connection closes before sending expected bytes"
    );
    let err_msg = terminal.error.unwrap_or_default();
    assert!(
        err_msg.contains("Premature EOF")
            || err_msg.contains("exhausted")
            || err_msg.contains("incomplete"),
        "Error should reflect premature stream termination: {}",
        err_msg
    );

    server.stop();
}

/// d) Final completeness mismatch:
/// When body is truncated and server closes without error, download_inner's final
/// completeness check prevents job from being marked Completed.
#[tokio::test]
async fn test_final_completeness_mismatch_prevents_completion() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 512 * 1024; // 512 KB expected from HEAD
    let source_data = generate_deterministic_payload(total_size);
    // Server closes after 64 KB
    let server = MockServer::start(
        source_data,
        ServerMode::CloseAfter {
            bytes_to_send: 64 * 1024,
        },
    )
    .await;

    let filename = "test_completeness_mismatch.bin";
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 1);

    let task = DownloadTask::start_with_id(task_id, req);
    let terminal = wait_for_terminal_state(&task, Duration::from_secs(8)).await;

    assert_ne!(
        terminal.state,
        TaskState::Completed,
        "Download must NEVER be marked Completed if bytes or ranges are missing"
    );
    assert_eq!(terminal.state, TaskState::Failed);

    server.stop();
}

/// e) Valid 206 with correct Content-Range still succeeds:
/// Standard multi-chunk download finishes and verifies SHA-256 byte-for-byte.
#[tokio::test]
async fn test_valid_206_with_content_range_succeeds() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 2 * 1024 * 1024; // 2 MB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data.clone(), ServerMode::Normal).await;

    let filename = "test_valid_206.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 4);

    let task = DownloadTask::start_with_id(task_id, req);
    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;

    assert_eq!(completed.state, TaskState::Completed);
    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}

/// f) Fresh non-ranged 200 still succeeds:
/// Single-stream download where server does not support ranges succeeds with HTTP 200
/// and verifies SHA-256 byte-for-byte.
#[tokio::test]
async fn test_fresh_non_ranged_200_succeeds() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 512 * 1024; // 512 KB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data.clone(), ServerMode::NonRanged).await;

    let filename = "test_non_ranged_200.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 1);

    let task = DownloadTask::start_with_id(task_id, req);
    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;

    assert_eq!(completed.state, TaskState::Completed);
    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}
