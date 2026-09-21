use std::{
    collections::VecDeque,
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
    sync::{Mutex, RwLock},
};
use uuid::Uuid;
use vajra_engine::{
    db::{Database, JobRecord},
    download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskState},
    multiplexer::MultiplexerOptions,
};

static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

// ─── Flexible Mock HTTP Server for Retry & Timeout Testing ──────────────────

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum AttemptBehavior {
    /// Return standard response (200 for non-ranged/HEAD, 206 for ranged)
    Normal,
    /// Return specific HTTP status with optional headers and body
    CustomStatus {
        status: u16,
        status_text: &'static str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    /// Send response headers, send `bytes_to_send`, then abruptly close connection
    PartialThenDrop { bytes_to_send: usize },
    /// Send response headers, then trickle body with chunk delay
    SlowTrickle { chunk_size: usize, delay_ms: u64 },
    /// Send response headers, send `initial_bytes`, then stall for `stall_ms`
    StallAfter { initial_bytes: usize, stall_ms: u64 },
    /// Abruptly drop TCP connection immediately without sending headers
    ImmediateDrop,
    /// Delay sending headers by `delay_ms`
    DelayHeaders { delay_ms: u64 },
}

#[derive(Clone, Default)]
struct ServerConfig {
    data: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    date: Option<String>,
    accept_ranges: bool,
    head_status: Option<u16>,
    probe_delay_ms: u64,
    behaviors: VecDeque<AttemptBehavior>,
}

struct MockServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    recorded_requests: Arc<Mutex<Vec<String>>>,
    get_timestamps: Arc<Mutex<Vec<Instant>>>,
}

impl MockServer {
    async fn start(config: ServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let config_shared = Arc::new(RwLock::new(config));
        let config_clone = Arc::clone(&config_shared);
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let recorded_clone = Arc::clone(&recorded);
        let get_ts = Arc::new(Mutex::new(Vec::new()));
        let get_ts_clone = Arc::clone(&get_ts);

        tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let cfg_lock = Arc::clone(&config_clone);
                let rec_lock = Arc::clone(&recorded_clone);
                let get_ts_lock = Arc::clone(&get_ts_clone);

                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req_str = String::from_utf8_lossy(&buf[..n]).to_string();
                    let first_line = req_str.lines().next().unwrap_or("");
                    let is_head = first_line.starts_with("HEAD");
                    {
                        let mut rec = rec_lock.lock().await;
                        rec.push(req_str.clone());
                        if !is_head {
                            let mut gts = get_ts_lock.lock().await;
                            gts.push(Instant::now());
                        }
                    }

                    let mut range: Option<(usize, usize)> = None;
                    for line in req_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("range:") {
                            if let Some(bytes_part) = line.split('=').nth(1) {
                                let parts: Vec<&str> = bytes_part.trim().split('-').collect();
                                if let Ok(start) = parts[0].parse::<usize>() {
                                    let end = if parts.len() > 1 && !parts[1].is_empty() {
                                        parts[1].parse::<usize>().unwrap_or(usize::MAX)
                                    } else {
                                        usize::MAX
                                    };
                                    range = Some((start, end));
                                }
                            }
                        }
                    }

                    let (cfg, behavior) = {
                        let mut guard = cfg_lock.write().await;
                        let beh = if !is_head {
                            guard
                                .behaviors
                                .pop_front()
                                .unwrap_or(AttemptBehavior::Normal)
                        } else {
                            AttemptBehavior::Normal
                        };
                        (guard.clone(), beh)
                    };

                    let total_len = cfg.data.len();

                    // ── Handle HEAD requests ────────────────────────────────
                    if is_head {
                        if cfg.probe_delay_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(cfg.probe_delay_ms)).await;
                        }
                        let head_code = cfg.head_status.unwrap_or(200);
                        if head_code == 405 {
                            let resp = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                            let _ = socket.write_all(resp.as_bytes()).await;
                            return;
                        }
                        let mut resp = format!(
                            "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                            head_code, total_len
                        );
                        if cfg.accept_ranges {
                            resp.push_str("Accept-Ranges: bytes\r\n");
                        }
                        if let Some(ref etag) = cfg.etag {
                            resp.push_str(&format!("ETag: {}\r\n", etag));
                        }
                        if let Some(ref lm) = cfg.last_modified {
                            resp.push_str(&format!("Last-Modified: {}\r\n", lm));
                        }
                        if let Some(ref d) = cfg.date {
                            resp.push_str(&format!("Date: {}\r\n", d));
                        }
                        resp.push_str("Connection: close\r\n\r\n");
                        let _ = socket.write_all(resp.as_bytes()).await;
                        return;
                    }

                    // ── Handle GET behaviors ────────────────────────────────
                    if range == Some((0, 0)) && cfg.probe_delay_ms > 0 {
                        tokio::time::sleep(Duration::from_millis(cfg.probe_delay_ms)).await;
                    }
                    match behavior {
                        AttemptBehavior::ImmediateDrop => {
                            // Drop socket connection immediately
                            drop(socket);
                        }
                        AttemptBehavior::DelayHeaders { delay_ms } => {
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                total_len
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(&cfg.data).await;
                        }
                        AttemptBehavior::CustomStatus {
                            status,
                            status_text,
                            headers,
                            body,
                        } => {
                            let mut resp = format!(
                                "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                status,
                                status_text,
                                body.len()
                            );
                            for (k, v) in headers {
                                resp.push_str(&format!("{}: {}\r\n", k, v));
                            }
                            resp.push_str("Connection: close\r\n\r\n");
                            let _ = socket.write_all(resp.as_bytes()).await;
                            if !body.is_empty() {
                                let _ = socket.write_all(&body).await;
                            }
                        }
                        AttemptBehavior::PartialThenDrop { bytes_to_send } => {
                            let (start, mut end) =
                                range.unwrap_or((0, total_len.saturating_sub(1)));
                            if end >= total_len {
                                end = total_len.saturating_sub(1);
                            }
                            let chunk_len = end.saturating_sub(start) + 1;
                            let mut resp = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                start, end, total_len, chunk_len
                            );
                            if let Some(ref etag) = cfg.etag {
                                resp.push_str(&format!("ETag: {}\r\n", etag));
                            }
                            if let Some(ref lm) = cfg.last_modified {
                                resp.push_str(&format!("Last-Modified: {}\r\n", lm));
                            }
                            resp.push_str("Connection: close\r\n\r\n");
                            let _ = socket.write_all(resp.as_bytes()).await;

                            let slice = &cfg.data[start..=end];
                            let to_write = bytes_to_send.min(slice.len());
                            let _ = socket.write_all(&slice[..to_write]).await;
                            let _ = socket.flush().await;
                            // Drop socket to simulate abrupt connection drop
                            drop(socket);
                        }
                        AttemptBehavior::SlowTrickle {
                            chunk_size,
                            delay_ms,
                        } => {
                            let (start, mut end) =
                                range.unwrap_or((0, total_len.saturating_sub(1)));
                            if end >= total_len {
                                end = total_len.saturating_sub(1);
                            }
                            let chunk_len = end.saturating_sub(start) + 1;
                            let mut resp = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                start, end, total_len, chunk_len
                            );
                            if let Some(ref etag) = cfg.etag {
                                resp.push_str(&format!("ETag: {}\r\n", etag));
                            }
                            resp.push_str("Connection: close\r\n\r\n");
                            let _ = socket.write_all(resp.as_bytes()).await;

                            let slice = &cfg.data[start..=end];
                            for piece in slice.chunks(chunk_size) {
                                if socket.write_all(piece).await.is_err() {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            }
                        }
                        AttemptBehavior::StallAfter {
                            initial_bytes,
                            stall_ms,
                        } => {
                            let (start, mut end) =
                                range.unwrap_or((0, total_len.saturating_sub(1)));
                            if end >= total_len {
                                end = total_len.saturating_sub(1);
                            }
                            let chunk_len = end.saturating_sub(start) + 1;
                            let mut resp = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                start, end, total_len, chunk_len
                            );
                            if let Some(ref etag) = cfg.etag {
                                resp.push_str(&format!("ETag: {}\r\n", etag));
                            }
                            resp.push_str("Connection: close\r\n\r\n");
                            let _ = socket.write_all(resp.as_bytes()).await;

                            let slice = &cfg.data[start..=end];
                            let to_write = initial_bytes.min(slice.len());
                            if to_write > 0 {
                                let _ = socket.write_all(&slice[..to_write]).await;
                                let _ = socket.flush().await;
                            }
                            // Stall without closing
                            tokio::time::sleep(Duration::from_millis(stall_ms)).await;
                        }
                        AttemptBehavior::Normal => {
                            if let Some((start, mut end)) = range {
                                if end >= total_len {
                                    end = total_len.saturating_sub(1);
                                }
                                if start >= total_len {
                                    let resp = format!(
                                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nConnection: close\r\n\r\n",
                                        total_len
                                    );
                                    let _ = socket.write_all(resp.as_bytes()).await;
                                    return;
                                }
                                let chunk_len = end.saturating_sub(start) + 1;
                                let mut resp = format!(
                                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                    start, end, total_len, chunk_len
                                );
                                if cfg.accept_ranges {
                                    resp.push_str("Accept-Ranges: bytes\r\n");
                                }
                                if let Some(ref etag) = cfg.etag {
                                    resp.push_str(&format!("ETag: {}\r\n", etag));
                                }
                                if let Some(ref lm) = cfg.last_modified {
                                    resp.push_str(&format!("Last-Modified: {}\r\n", lm));
                                }
                                if let Some(ref d) = cfg.date {
                                    resp.push_str(&format!("Date: {}\r\n", d));
                                }
                                resp.push_str("Connection: close\r\n\r\n");
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&cfg.data[start..=end]).await;
                            } else {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                                    total_len
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&cfg.data).await;
                            }
                        }
                    }
                });
            }
        });

        MockServer {
            addr,
            shutdown,
            recorded_requests: recorded,
            get_timestamps: get_ts,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/resource.bin", self.addr)
    }

    async fn get_recorded_requests(&self) -> Vec<String> {
        let guard = self.recorded_requests.lock().await;
        guard.clone()
    }

    async fn get_get_request_timestamps(&self) -> Vec<Instant> {
        let guard = self.get_timestamps.lock().await;
        guard.clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

async fn wait_for_terminal_state(
    task: &mut DownloadTask,
    timeout: Duration,
) -> Option<DownloadProgress> {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            return None;
        }
        let p = task.progress_rx.borrow().clone();
        match p.state {
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled | TaskState::Paused => {
                return Some(p);
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        }
    }
}

fn generate_test_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push(((i * 37 + 19) % 256) as u8);
    }
    data
}

fn test_mux_options() -> MultiplexerOptions {
    MultiplexerOptions {
        base_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(50),
        max_retry_after: Duration::from_millis(200),
        inactivity_timeout: Duration::from_millis(800),
        header_timeout: Duration::from_millis(800),
        max_retries: 4,
        max_total_attempts: 10,
        ..Default::default()
    }
}

// ─── 1. RETRY-AFTER TESTING ──────────────────────────────────────────────────

#[tokio::test]
async fn test_retry_after_delta_seconds_honored_and_capped() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 64 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: 429 Too Many Requests with Retry-After: 99999 (must be clamped to max_retry_after = 200ms)
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 429,
        status_text: "Too Many Requests",
        headers: vec![("Retry-After".to_string(), "99999".to_string())],
        body: b"Rate limited".to_vec(),
    });
    // Attempt 2: Normal 206 success
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("retry_after_clamped.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("retry_after_clamped.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let start_time = Instant::now();
    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    let elapsed = start_time.elapsed();

    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));
    // The test must NOT have slept for 99999 seconds! It should complete in under 2 seconds.
    assert!(
        elapsed < Duration::from_secs(2),
        "Download took too long: {:?}",
        elapsed
    );

    let timestamps = server.get_get_request_timestamps().await;
    // We expect attempt 1 GET, then attempt 2 GET
    assert!(timestamps.len() >= 2);
    // Diff between attempt 1 and attempt 2 should be at least ~150ms (clamped to 200ms)
    let get_ts1 = timestamps[0];
    let get_ts2 = timestamps[1];
    let gap = get_ts2.duration_since(get_ts1);
    assert!(
        gap >= Duration::from_millis(150),
        "Retry-After clamp violated: gap was {:?}",
        gap
    );

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

#[tokio::test]
async fn test_retry_after_http_date_format() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 32 * 1024;
    let data = generate_test_data(size);

    // Generate IMF-fixdate 1 second in the future
    let future_date = chrono::Utc::now() + chrono::Duration::seconds(1);
    let imf_date = future_date.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 503,
        status_text: "Service Unavailable",
        headers: vec![("Retry-After".to_string(), imf_date)],
        body: b"Busy".to_vec(),
    });
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("retry_after_date.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("retry_after_date.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

// ─── 2. PARTIAL-PROGRESS RETRY SAFETY ───────────────────────────────────────

#[tokio::test]
async fn test_partial_progress_retry_resumes_from_correct_byte_and_matches_sha256() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 64 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: sends exactly 24KB then drops connection abruptly
    behaviors.push_back(AttemptBehavior::PartialThenDrop {
        bytes_to_send: 24 * 1024,
    });
    // Attempt 2: succeeds normally for remaining range
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("partial_retry.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("partial_retry.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(written.len(), size);
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));

    // Verify request ranges: attempt 1 requested bytes=0-65535, attempt 2 requested bytes=24576-65535
    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    assert_eq!(get_reqs.len(), 2);
    assert!(get_reqs[0].contains("bytes=0-65535"));
    assert!(get_reqs[1].contains("bytes=24576-65535"));
}

#[tokio::test]
async fn test_multiple_partial_failures_followed_by_success() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 96 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: emits 16KB then drops
    behaviors.push_back(AttemptBehavior::PartialThenDrop {
        bytes_to_send: 16 * 1024,
    });
    // Attempt 2: emits 32KB more then drops
    behaviors.push_back(AttemptBehavior::PartialThenDrop {
        bytes_to_send: 32 * 1024,
    });
    // Attempt 3: succeeds for remaining bytes
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("multi_partial.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("multi_partial.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(8)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(written.len(), size);
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));

    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    assert_eq!(get_reqs.len(), 3);
    assert!(get_reqs[0].contains("bytes=0-98303"));
    assert!(get_reqs[1].contains("bytes=16384-98303"));
    assert!(get_reqs[2].contains("bytes=49152-98303"));
}

// ─── 3. STREAM INACTIVITY TIMEOUT ───────────────────────────────────────────

#[tokio::test]
async fn test_slow_progressing_stream_survives_cumulative_duration_longer_than_timeout() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 32 * 1024;
    let data = generate_test_data(size);

    // Stream 8 chunks of 4KB with 60ms delay between chunks.
    // Total stream duration = 8 * 60ms = ~480ms.
    // Inactivity timeout is set to 250ms.
    // Since each chunk arrives in 60ms < 250ms, download MUST succeed!
    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::SlowTrickle {
        chunk_size: 4 * 1024,
        delay_ms: 60,
    });

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let mut mux_opts = test_mux_options();
    mux_opts.inactivity_timeout = Duration::from_millis(250);

    let dest_file = temp_dir.path().join("slow_stream.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("slow_stream.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(mux_opts),
        ..Default::default()
    };

    let start = Instant::now();
    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    let elapsed = start.elapsed();

    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));
    assert!(
        elapsed >= Duration::from_millis(400),
        "Should have taken >= 400ms, took {:?}",
        elapsed
    );

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

#[tokio::test]
async fn test_genuinely_stalled_stream_times_out() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 32 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: sends 4KB then stalls for 3000ms (exceeding 200ms inactivity timeout)
    behaviors.push_back(AttemptBehavior::StallAfter {
        initial_bytes: 4 * 1024,
        stall_ms: 3000,
    });
    // Attempt 2: succeeds normally
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let mut mux_opts = test_mux_options();
    mux_opts.inactivity_timeout = Duration::from_millis(200);

    let dest_file = temp_dir.path().join("stalled_stream.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("stalled_stream.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(mux_opts),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));

    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    assert_eq!(get_reqs.len(), 2);
    assert!(get_reqs[1].contains("bytes=4096-32767"));
}

// ─── 4. TRANSIENT STATUS CODE RETRIES ────────────────────────────────────────

#[tokio::test]
async fn test_http_429_retries_and_succeeds() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 16 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 429,
        status_text: "Too Many Requests",
        headers: vec![],
        body: b"Rate limited".to_vec(),
    });
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("retry_429.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("retry_429.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

#[tokio::test]
async fn test_http_503_retries_and_succeeds() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 16 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 503,
        status_text: "Service Unavailable",
        headers: vec![],
        body: b"Service busy".to_vec(),
    });
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("retry_503.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("retry_503.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

#[tokio::test]
async fn test_http_502_504_retries_and_succeeds() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 16 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 502,
        status_text: "Bad Gateway",
        headers: vec![],
        body: b"Bad Gateway".to_vec(),
    });
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 504,
        status_text: "Gateway Timeout",
        headers: vec![],
        body: b"Gateway Timeout".to_vec(),
    });
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("retry_502_504.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("retry_502_504.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

// ─── 5. PERMANENT NON-RETRYABLE ERRORS ──────────────────────────────────────

#[tokio::test]
async fn test_permanent_404_does_not_retry() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 404,
        status_text: "Not Found",
        headers: vec![],
        body: b"Missing".to_vec(),
    });

    let server = MockServer::start(ServerConfig {
        data: vec![0; 1024],
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("not_found.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Failed));

    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    // Exactly 1 GET attempt, no retries!
    assert_eq!(get_reqs.len(), 1);
}

#[tokio::test]
async fn test_416_does_not_retry() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let mut behaviors = VecDeque::new();
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 416,
        status_text: "Range Not Satisfiable",
        headers: vec![("Content-Range".to_string(), "bytes */1024".to_string())],
        body: vec![],
    });

    let server = MockServer::start(ServerConfig {
        data: vec![0; 1024],
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("range_not_sat.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Failed));

    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    // Exactly 1 GET attempt, no retries!
    assert_eq!(get_reqs.len(), 1);
}

// ─── 6. RESOURCE IDENTITY & IF-RANGE PRESERVATION ───────────────────────────

#[tokio::test]
async fn test_retry_preserves_if_range_validator() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 32 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: 503 Service Unavailable
    behaviors.push_back(AttemptBehavior::CustomStatus {
        status: 503,
        status_text: "Service Unavailable",
        headers: vec![],
        body: b"Busy".to_vec(),
    });
    // Attempt 2: Normal 206
    behaviors.push_back(AttemptBehavior::Normal);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"strong-etag-v1\"".to_string()),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("if_range_retry.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("if_range_retry.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    assert_eq!(get_reqs.len(), 2);

    // Both attempt 1 and attempt 2 must carry the exact strong ETag in If-Range!
    assert!(
        get_reqs[0].contains("if-range: \"strong-etag-v1\"")
            || get_reqs[0].contains("If-Range: \"strong-etag-v1\"")
    );
    assert!(
        get_reqs[1].contains("if-range: \"strong-etag-v1\"")
            || get_reqs[1].contains("If-Range: \"strong-etag-v1\"")
    );

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));
}

// ─── 7. RETRY EXHAUSTION & WRITER-CONFIRMED PERSISTENCE ─────────────────────

#[tokio::test]
async fn test_retry_exhaustion_persists_writer_confirmed_bytes() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 64 * 1024;
    let data = generate_test_data(size);

    let mut behaviors = VecDeque::new();
    // Attempt 1: emits 16KB then drops
    behaviors.push_back(AttemptBehavior::PartialThenDrop {
        bytes_to_send: 16 * 1024,
    });
    // Attempt 2: drops immediately (no progress)
    behaviors.push_back(AttemptBehavior::ImmediateDrop);
    // Attempt 3: drops immediately (no progress -> consecutive_failures reaches 2)
    behaviors.push_back(AttemptBehavior::ImmediateDrop);

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        behaviors,
        ..Default::default()
    })
    .await;

    let mut mux_opts = test_mux_options();
    mux_opts.max_retries = 2;

    let job_id = Uuid::new_v4();
    let dest_file = temp_dir.path().join("exhaustion.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("exhaustion.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(mux_opts),
        ..Default::default()
    };

    // Pre-insert job record in DB
    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: serde_json::to_string(&req).unwrap(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    let mut task = DownloadTask::start_with_id(job_id, req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Failed));

    // Verify SQLite persistence: exactly 16384 bytes confirmed written to disk
    let segments = db.load_segments(&job_id.to_string()).unwrap();
    assert!(!segments.is_empty());
    assert_eq!(segments[0].bytes_written, 16 * 1024);

    let disk_size = std::fs::metadata(&dest_file).map(|m| m.len()).unwrap_or(0);
    assert_eq!(disk_size, 64 * 1024); // pre-allocated
}

// ─── 8. PROBE TIMEOUT & BODY DROPPING ───────────────────────────────────────

#[tokio::test]
async fn test_probe_request_header_timeout_bounded() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    // Server delays probe response by 3000ms
    let server = MockServer::start(ServerConfig {
        data: vec![0; 1024],
        accept_ranges: true,
        probe_delay_ms: 3000,
        ..Default::default()
    })
    .await;

    let mut mux_opts = test_mux_options();
    mux_opts.header_timeout = Duration::from_millis(150);

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("probe_timeout.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(mux_opts),
        ..Default::default()
    };

    let start = Instant::now();
    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(3)).await;
    let elapsed = start.elapsed();

    // Probe timeout must trigger well before 3000ms!
    assert!(
        elapsed < Duration::from_millis(1500),
        "Probe took too long: {:?}",
        elapsed
    );
    // Since HEAD timed out and fallback GET also times out, task fails
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Failed));
}

#[tokio::test]
async fn test_probe_request_fallback_drops_body() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 32 * 1024;
    let data = generate_test_data(size);

    // HEAD returns 405 Method Not Allowed, forcing probe to do fallback GET (bytes=0-0)
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        accept_ranges: true,
        head_status: Some(405),
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("fallback_probe.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("fallback_probe.bin".to_string()),
        max_connections: 1,
        multiplexer_options: Some(test_mux_options()),
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let terminal = wait_for_terminal_state(&mut task, Duration::from_secs(5)).await;
    assert_eq!(terminal.map(|t| t.state), Some(TaskState::Completed));

    let written = std::fs::read(&dest_file).unwrap();
    assert_eq!(Sha256::digest(&written), Sha256::digest(&data));

    // Verify probe executed HEAD then GET bytes=0-0 then actual ranged GET
    let reqs = server.get_recorded_requests().await;
    let get_reqs: Vec<_> = reqs.iter().filter(|r| r.starts_with("GET")).collect();
    assert_eq!(get_reqs.len(), 2);
    assert!(get_reqs[0].contains("bytes=0-0"));
    assert!(get_reqs[1].contains("bytes=0-32767"));
}
