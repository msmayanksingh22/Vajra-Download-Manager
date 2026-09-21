use std::{
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
    sync::RwLock,
};
use uuid::Uuid;
use vajra_engine::{
    db::{Database, JobRecord},
    download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskState},
};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ─── Flexible Mock HTTP Server for Remote Resource Identity Validation ────────

#[derive(Clone, Default)]
struct ServerConfig {
    data: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    date: Option<String>,
    accept_ranges: bool,
    force_200_on_range: bool,
    chunk_delay_ms: u64,
    override_content_range_total: Option<u64>,
    content_range_star_total: bool,
    switch_after_range_requests: Option<(usize, Arc<ServerConfig>)>,
}

struct MockServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    config: Arc<RwLock<ServerConfig>>,
    recorded_requests: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl MockServer {
    async fn start(config: ServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let config_shared = Arc::new(RwLock::new(config));
        let config_clone = Arc::clone(&config_shared);
        let recorded = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let recorded_clone = Arc::clone(&recorded);
        let range_request_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let range_request_count_clone = Arc::clone(&range_request_count);

        tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let cfg_lock = Arc::clone(&config_clone);
                let rec_lock = Arc::clone(&recorded_clone);
                let range_cnt = Arc::clone(&range_request_count_clone);

                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req_str = String::from_utf8_lossy(&buf[..n]).to_string();
                    {
                        let mut rec = rec_lock.lock().await;
                        rec.push(req_str.clone());
                    }

                    let first_line = req_str.lines().next().unwrap_or("");
                    let is_head = first_line.starts_with("HEAD");

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

                    let mut cfg = cfg_lock.read().await.clone();
                    if range.is_some() {
                        let count = range_cnt.fetch_add(1, Ordering::SeqCst);
                        if let Some((threshold, ref next_cfg)) = cfg.switch_after_range_requests {
                            if count >= threshold {
                                cfg = (**next_cfg).clone();
                            }
                        }
                    }
                    let total_len = cfg.data.len();

                    if is_head {
                        let mut resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                            total_len
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

                    // GET request
                    if cfg.force_200_on_range && range.is_some() {
                        // Force 200 OK even when Range requested (simulate validator failure condition)
                        let mut resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                            total_len
                        );
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
                        let _ = socket.write_all(&cfg.data).await;
                        return;
                    }

                    if let Some((start, mut end)) = range {
                        if !cfg.accept_ranges {
                            // Server does not support ranges: return 200
                            let mut resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                                total_len
                            );
                            resp.push_str("Connection: close\r\n\r\n");
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(&cfg.data).await;
                            return;
                        }

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
                        let total_part = if cfg.content_range_star_total {
                            "*".to_string()
                        } else if let Some(t) = cfg.override_content_range_total {
                            format!("{t}")
                        } else {
                            format!("{total_len}")
                        };
                        let mut resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                            start, end, total_part, chunk_len
                        );
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
                        let slice = &cfg.data[start..=end];
                        if cfg.chunk_delay_ms > 0 {
                            for (i, piece) in slice.chunks(16 * 1024).enumerate() {
                                if socket.write_all(piece).await.is_err() {
                                    break;
                                }
                                if i > 0 {
                                    tokio::time::sleep(Duration::from_millis(cfg.chunk_delay_ms))
                                        .await;
                                }
                            }
                        } else {
                            let _ = socket.write_all(slice).await;
                        }
                    } else {
                        // Full GET request
                        let mut resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                            total_len
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
                        let _ = socket.write_all(&cfg.data).await;
                    }
                });
            }
        });

        MockServer {
            addr,
            shutdown,
            config: config_shared,
            recorded_requests: recorded,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/resource.bin", self.addr)
    }

    async fn set_config(&self, config: ServerConfig) {
        let mut guard = self.config.write().await;
        *guard = config;
    }

    async fn get_recorded_requests(&self) -> Vec<String> {
        let guard = self.recorded_requests.lock().await;
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
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

fn generate_test_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push(((i * 31 + 17) % 256) as u8);
    }
    data
}

// ─── Test Cases ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_saved_etag_same_etag_resumes() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data = generate_test_data(size);
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"etag-v1\"".to_string()),
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();

    // Pre-seed half download in SQLite and file
    let half = (size / 2) as u64;
    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    // Chunk 0: 0..half-1, complete
    db.save_segment(&job_id.to_string(), 0, 0, half - 1, half)
        .unwrap();
    // Chunk 1: half..size-1, 0 written
    db.save_segment(&job_id.to_string(), 1, half, (size - 1) as u64, 0)
        .unwrap();
    // Save strong etag validator
    db.save_validators(&job_id.to_string(), Some("etag"), Some("\"etag-v1\""), None)
        .unwrap();

    let dest_file = temp_dir.path().join("test_file.bin");
    // Write partial content to disk
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&dest_file).unwrap();
        f.write_all(&data[..half as usize]).unwrap();
        // preallocate remainder to simulate pre-allocation
        f.set_len(size as u64).unwrap();
    }

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("test_file.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Completed);

    // Verify downloaded file matches byte-for-byte
    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));

    // Verify If-Range was sent with the strong ETag
    let reqs = server.get_recorded_requests().await;
    let if_range_found = reqs
        .iter()
        .any(|r| r.contains("If-Range: \"etag-v1\"") || r.contains("if-range: \"etag-v1\""));
    assert!(
        if_range_found,
        "Server should have received If-Range header with ETag"
    );
}

#[tokio::test]
async fn test_saved_etag_changed_etag_fails_and_resets() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data = generate_test_data(size);
    // Server now has etag-v2!
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"etag-v2\"".to_string()),
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();
    let half = (size / 2) as u64;

    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    db.save_segment(&job_id.to_string(), 0, 0, half - 1, half)
        .unwrap();
    db.save_segment(&job_id.to_string(), 1, half, (size - 1) as u64, 0)
        .unwrap();
    // Saved validator is etag-v1!
    db.save_validators(&job_id.to_string(), Some("etag"), Some("\"etag-v1\""), None)
        .unwrap();

    let dest_file = temp_dir.path().join("changed_etag.bin");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&dest_file).unwrap();
        f.write_all(&vec![0xAA; half as usize]).unwrap();
    }

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("changed_etag.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Failed);
    let err_msg = final_progress.error.unwrap_or_default();
    assert!(err_msg.contains("ETag changed") || err_msg.contains("Resource changed"));

    // DB segments must be wiped
    let remaining_segments = db.load_segments(&job_id.to_string()).unwrap();
    assert!(
        remaining_segments.is_empty(),
        "Segments must be deleted after validator mismatch"
    );

    // DB validators must be wiped
    let remaining_validators = db.load_validators(&job_id.to_string()).unwrap();
    assert!(
        remaining_validators.is_none(),
        "Validators must be deleted after validator mismatch"
    );

    // Partial file must be deleted
    assert!(
        !dest_file.exists(),
        "Partial file must be deleted after validator mismatch"
    );
}

#[tokio::test]
async fn test_saved_etag_missing_current_etag_fails_closed() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 128 * 1024;
    let data = generate_test_data(size);
    // Server has NO ETag anymore
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: None,
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();

    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    db.save_segment(&job_id.to_string(), 0, 0, (size - 1) as u64, 500)
        .unwrap();
    db.save_validators(&job_id.to_string(), Some("etag"), Some("\"etag-v1\""), None)
        .unwrap();

    let dest_file = temp_dir.path().join("missing_etag.bin");
    std::fs::File::create(&dest_file).unwrap();

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("missing_etag.bin".to_string()),
        max_connections: 1,
        ..Default::default()
    };

    let mut task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Failed);
    let err_msg = final_progress.error.unwrap_or_default();
    assert!(err_msg.contains("no longer provides an ETag"));

    // Segments and validators deleted
    assert!(db.load_segments(&job_id.to_string()).unwrap().is_empty());
    assert!(db.load_validators(&job_id.to_string()).unwrap().is_none());
    assert!(!dest_file.exists());
}

#[tokio::test]
async fn test_saved_last_modified_same_last_modified_resumes() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 200 * 1024;
    let data = generate_test_data(size);
    let lm_date = "Sun, 06 Nov 1994 08:40:00 GMT";
    let now_date = "Sun, 06 Nov 1994 08:49:37 GMT"; // diff > 60s -> strong

    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: None,
        last_modified: Some(lm_date.to_string()),
        date: Some(now_date.to_string()),
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();
    let half = (size / 2) as u64;

    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    db.save_segment(&job_id.to_string(), 0, 0, half - 1, half)
        .unwrap();
    db.save_segment(&job_id.to_string(), 1, half, (size - 1) as u64, 0)
        .unwrap();
    db.save_validators(
        &job_id.to_string(),
        Some("last_modified"),
        None,
        Some(lm_date),
    )
    .unwrap();

    let dest_file = temp_dir.path().join("resume_lm.bin");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&dest_file).unwrap();
        f.write_all(&data[..half as usize]).unwrap();
        f.set_len(size as u64).unwrap();
    }

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("resume_lm.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Completed);

    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));

    // Verify If-Range was sent with the Last-Modified date
    let reqs = server.get_recorded_requests().await;
    let if_range_found = reqs.iter().any(|r| {
        r.contains(&format!("If-Range: {}", lm_date))
            || r.contains(&format!("if-range: {}", lm_date))
    });
    assert!(
        if_range_found,
        "Server should have received If-Range header with Last-Modified"
    );
}

#[tokio::test]
async fn test_selected_last_modified_used_as_if_range() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 150 * 1024;
    let data = generate_test_data(size);
    let lm_date = "Sun, 06 Nov 1994 08:40:00 GMT";
    let now_date = "Sun, 06 Nov 1994 08:49:37 GMT";
    // Weak ETag + strong Last-Modified
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("W/\"weak-tag\"".to_string()),
        last_modified: Some(lm_date.to_string()),
        date: Some(now_date.to_string()),
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("weak_etag_strong_lm.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Completed);

    // Verify that If-Range contains the strong Last-Modified, NEVER the weak ETag
    let reqs = server.get_recorded_requests().await;
    for r in &reqs {
        assert!(
            !r.contains("If-Range: W/\"weak-tag\""),
            "Weak ETag must NEVER be sent as If-Range"
        );
        assert!(
            !r.contains("if-range: W/\"weak-tag\""),
            "Weak ETag must NEVER be sent as If-Range"
        );
    }
}

#[tokio::test]
async fn test_different_etag_not_rejected_when_last_modified_is_selected() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 200 * 1024;
    let data = generate_test_data(size);
    let lm_date = "Sun, 06 Nov 1994 08:40:00 GMT";
    let now_date = "Sun, 06 Nov 1994 08:49:37 GMT";

    // Resumed probe returns same Last-Modified, but now ALSO returns a new/different ETag
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"brand-new-etag\"".to_string()),
        last_modified: Some(lm_date.to_string()),
        date: Some(now_date.to_string()),
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();
    let half = (size / 2) as u64;

    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    db.save_segment(&job_id.to_string(), 0, 0, half - 1, half)
        .unwrap();
    db.save_segment(&job_id.to_string(), 1, half, (size - 1) as u64, 0)
        .unwrap();
    // Selected validator is last_modified
    db.save_validators(
        &job_id.to_string(),
        Some("last_modified"),
        None,
        Some(lm_date),
    )
    .unwrap();

    let dest_file = temp_dir.path().join("lm_different_etag.bin");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&dest_file).unwrap();
        f.write_all(&data[..half as usize]).unwrap();
        f.set_len(size as u64).unwrap();
    }

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("lm_different_etag.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    // Must NOT reject!
    assert_eq!(final_progress.state, TaskState::Completed);

    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));
}

#[tokio::test]
async fn test_validator_mismatch_leaves_no_old_resource_bytes() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    // Resource 1: 500 KB filled with 0xAA
    let res1_size = 500 * 1024;
    let res1_data = vec![0xAAu8; res1_size];

    // Resource 2: 200 KB filled with 0xBB (smaller than resource 1!)
    let res2_size = 200 * 1024;
    let res2_data = vec![0xBBu8; res2_size];

    // Server starts hosting resource 2 with ETag v2
    let server = MockServer::start(ServerConfig {
        data: res2_data.clone(),
        etag: Some("\"etag-v2\"".to_string()),
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let db = Database::open(&vajra_protocol::db_path()).unwrap();
    let job_id = Uuid::new_v4();

    // Seed partial download of Resource 1: 250 KB written
    let res1_half = (res1_size / 2) as u64;
    db.upsert_job(&JobRecord {
        id: job_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    db.save_segment(&job_id.to_string(), 0, 0, res1_half - 1, res1_half)
        .unwrap();
    db.save_segment(&job_id.to_string(), 1, res1_half, (res1_size - 1) as u64, 0)
        .unwrap();
    db.save_validators(&job_id.to_string(), Some("etag"), Some("\"etag-v1\""), None)
        .unwrap();

    let dest_file = temp_dir.path().join("mixed_check.bin");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&dest_file).unwrap();
        // Write old 0xAA bytes
        f.write_all(&res1_data[..res1_half as usize]).unwrap();
        f.set_len(res1_size as u64).unwrap();
    }

    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("mixed_check.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    // Step 1: Attempt resumed download -> fails due to ETag mismatch
    let mut task = DownloadTask::start_with_id(job_id, req.clone());
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Failed);
    assert!(
        !dest_file.exists(),
        "Old physical file must be wiped on mismatch"
    );

    // Step 2: Fresh download of resource 2 with a new job ID (or fresh start)
    let fresh_id = Uuid::new_v4();
    let mut fresh_task = DownloadTask::start_with_id(fresh_id, req);
    let fresh_progress = wait_for_terminal_state(&mut fresh_task, Duration::from_secs(5))
        .await
        .expect("Fresh download should finish");

    assert_eq!(fresh_progress.state, TaskState::Completed);

    // Verify resulting file is strictly 200 KB and contains ONLY 0xBB bytes
    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), res2_size);
    assert_eq!(disk_data, res2_data);
    assert!(
        !disk_data.contains(&0xAA),
        "Old resource 0xAA bytes must NEVER survive"
    );
}

#[tokio::test]
async fn test_validator_survives_application_restart() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data = generate_test_data(size);
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"restart-v1\"".to_string()),
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 50,
        ..Default::default()
    })
    .await;

    let job_id = Uuid::new_v4();
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("restart_test.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    // Start download
    let mut task = DownloadTask::start_with_id(job_id, req.clone());
    // Wait until it reaches Downloading state and writes some bytes
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        let p = task.progress_rx.borrow().clone();
        if p.state == TaskState::Downloading && p.bytes_downloaded > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Pause download
    let _ = task.pause().await;
    let pause_prog = wait_for_terminal_state(&mut task, Duration::from_secs(3)).await;
    assert_eq!(pause_prog.map(|p| p.state), Some(TaskState::Paused));

    // Drop task handle simulating application shutdown
    drop(task);

    // Inspect SQLite database: verify validator record is persisted
    {
        let db = Database::open(&vajra_protocol::db_path()).unwrap();
        let val = db.load_validators(&job_id.to_string()).unwrap();
        assert!(val.is_some(), "Validator must be persisted in SQLite");
        let val = val.unwrap();
        assert_eq!(val.selected_type.as_deref(), Some("etag"));
        assert_eq!(val.etag.as_deref(), Some("\"restart-v1\""));
    }

    // Speed up server for resume
    server
        .set_config(ServerConfig {
            data: data.clone(),
            etag: Some("\"restart-v1\"".to_string()),
            last_modified: None,
            date: None,
            accept_ranges: true,
            force_200_on_range: false,
            chunk_delay_ms: 0,
            ..Default::default()
        })
        .await;

    // Simulate restart: start new DownloadTask with same job_id and request
    let mut resumed_task = DownloadTask::start_with_id(job_id, req);
    let final_progress = wait_for_terminal_state(&mut resumed_task, Duration::from_secs(5))
        .await
        .expect("Task should complete after restart");

    assert_eq!(final_progress.state, TaskState::Completed);

    let dest_file = temp_dir.path().join("restart_test.bin");
    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));
}

#[tokio::test]
async fn test_no_validator_server_continues_to_work() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 200 * 1024;
    let data = generate_test_data(size);
    // Server provides NO etag, NO last-modified
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: None,
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: false,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("no_validator.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("no_validator.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should complete");

    assert_eq!(final_progress.state, TaskState::Completed);

    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));
}

#[tokio::test]
async fn test_200_ok_on_if_range_fails_safely() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 200 * 1024;
    let data = generate_test_data(size);

    // Server force_200_on_range = true: returns 200 OK with full body when Range is sent!
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"etag-v1\"".to_string()),
        last_modified: None,
        date: None,
        accept_ranges: true,
        force_200_on_range: true,
        chunk_delay_ms: 0,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("force_200.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("force_200.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should complete or fail");

    // Must fail safely, not stream 200 into ranged destination!
    assert_eq!(final_progress.state, TaskState::Failed);
    let err = final_progress.error.unwrap_or_default();
    assert!(
        err.contains("200 OK received")
            || err.contains("remote resource identity condition failed")
            || err.contains("unexpected HTTP status 200")
    );
    assert!(
        !dest_file.exists(),
        "Partial file must not survive when 200 OK received on If-Range"
    );
}

#[tokio::test]
async fn test_content_range_contradictory_total_is_rejected() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data = generate_test_data(size);

    // Server probe returns 256 KB (size), but ranged 206 responses send total 999999!
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"etag-v1\"".to_string()),
        accept_ranges: true,
        override_content_range_total: Some(999999),
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("contradictory_total.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("contradictory_total.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Failed);
    let err = final_progress.error.unwrap_or_default();
    assert!(
        err.contains("Content-Range total mismatch") || err.contains("Resource changed"),
        "Error must report Content-Range total mismatch, got: {err}"
    );

    assert!(
        !dest_file.exists() || std::fs::metadata(&dest_file).map(|m| m.len()).unwrap_or(0) == 0,
        "Partial file must not contain written data on total mismatch"
    );
}

#[tokio::test]
async fn test_content_range_star_total_is_supported() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 128 * 1024;
    let data = generate_test_data(size);

    // Server sends Content-Range: bytes START-END/*
    let server = MockServer::start(ServerConfig {
        data: data.clone(),
        etag: Some("\"etag-v1\"".to_string()),
        accept_ranges: true,
        content_range_star_total: true,
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("star_total.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("star_total.bin".to_string()),
        max_connections: 1,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Completed);
    let disk_data = std::fs::read(&dest_file).unwrap();
    assert_eq!(disk_data.len(), size);
    assert_eq!(Sha256::digest(&disk_data), Sha256::digest(&data));
}

#[tokio::test]
async fn test_mid_flight_etag_change_rejects_and_resets() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data_v1 = vec![0xAAu8; size];
    let data_v2 = vec![0xBBu8; size];

    // Config for V2
    let v2_cfg = Arc::new(ServerConfig {
        data: data_v2.clone(),
        etag: Some("\"etag-v2\"".to_string()),
        accept_ranges: true,
        ..Default::default()
    });

    // Server starts with V1, but switches to V2 on range request index 1 (the 2nd range request)
    let server = MockServer::start(ServerConfig {
        data: data_v1.clone(),
        etag: Some("\"etag-v1\"".to_string()),
        accept_ranges: true,
        switch_after_range_requests: Some((1, v2_cfg)),
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("midflight_etag.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("midflight_etag.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    // Task must fail safely
    assert_eq!(final_progress.state, TaskState::Failed);
    let err = final_progress.error.unwrap_or_default();
    assert!(
        err.contains("ETag mismatch") || err.contains("Resource changed"),
        "Error must report ETag mismatch / resource changed, got: {err}"
    );

    // Destination file must NOT survive with mixed generation bytes
    assert!(
        !dest_file.exists(),
        "Partial file must be deleted on mid-flight validator mismatch; no mixed-generation bytes may survive"
    );
}

#[tokio::test]
async fn test_mid_flight_last_modified_change_rejects_and_resets() {
    let _guard = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let size = 256 * 1024;
    let data_v1 = vec![0x11u8; size];
    let data_v2 = vec![0x22u8; size];

    let lm_v1 = "Sun, 06 Nov 1994 08:40:00 GMT";
    let lm_v2 = "Sun, 06 Nov 1994 09:40:00 GMT";
    let date_hdr = "Sun, 06 Nov 1994 10:00:00 GMT";

    let v2_cfg = Arc::new(ServerConfig {
        data: data_v2.clone(),
        etag: None,
        last_modified: Some(lm_v2.to_string()),
        date: Some(date_hdr.to_string()),
        accept_ranges: true,
        ..Default::default()
    });

    let server = MockServer::start(ServerConfig {
        data: data_v1.clone(),
        etag: None,
        last_modified: Some(lm_v1.to_string()),
        date: Some(date_hdr.to_string()),
        accept_ranges: true,
        switch_after_range_requests: Some((1, v2_cfg)),
        ..Default::default()
    })
    .await;

    let dest_file = temp_dir.path().join("midflight_lm.bin");
    let req = DownloadRequest {
        url: server.url(),
        dest_dir: temp_dir.path().to_path_buf(),
        filename: Some("midflight_lm.bin".to_string()),
        max_connections: 2,
        ..Default::default()
    };

    let mut task = DownloadTask::start(req);
    let final_progress = wait_for_terminal_state(&mut task, Duration::from_secs(5))
        .await
        .expect("Task should finish");

    assert_eq!(final_progress.state, TaskState::Failed);
    let err = final_progress.error.unwrap_or_default();
    assert!(
        err.contains("Last-Modified mismatch") || err.contains("Resource changed"),
        "Error must report Last-Modified mismatch / resource changed, got: {err}"
    );

    assert!(
        !dest_file.exists(),
        "Partial file must be deleted on mid-flight Last-Modified mismatch"
    );
}
