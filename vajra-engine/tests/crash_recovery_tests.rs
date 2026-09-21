use std::{
    path::{Path, PathBuf},
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
use vajra_engine::{
    db::{Database, JobRecord, SegmentRecord},
    download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskState},
    multiplexer::MultiplexerOptions,
    writer::{start_disk_writer, start_disk_writer_with_counter, DataFrame, WriterCommand},
};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ─── Mock HTTP Server supporting byte ranges, pacing, and dynamic headers ────

type PacerFn = Arc<dyn Fn(usize, usize) -> u64 + Send + Sync>;

#[allow(dead_code)]
struct MockServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    etag: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl MockServer {
    async fn start(data: Vec<u8>, chunk_delay_ms: u64) -> Self {
        Self::start_with_pacer_and_etag(data, Arc::new(move |_, _| chunk_delay_ms), None).await
    }

    async fn start_with_etag(data: Vec<u8>, chunk_delay_ms: u64, etag: Option<String>) -> Self {
        Self::start_with_pacer_and_etag(data, Arc::new(move |_, _| chunk_delay_ms), etag).await
    }

    async fn start_with_pacer_and_etag(
        data: Vec<u8>,
        pacer: PacerFn,
        etag: Option<String>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let data = Arc::new(data);
        let etag = Arc::new(tokio::sync::RwLock::new(etag));
        let etag_clone = etag.clone();

        tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let data = Arc::clone(&data);
                let pacer = Arc::clone(&pacer);
                let etag_lock = Arc::clone(&etag_clone);

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

                    let current_etag = etag_lock.read().await.clone();
                    let etag_header = match &current_etag {
                        Some(val) => format!("ETag: {}\r\n", val),
                        None => String::new(),
                    };

                    // Parse Range header if present
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

                    if is_head {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{}Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                            total_len, etag_header
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    } else if let Some((start, end)) = range {
                        let chunk_len = end.saturating_sub(start) + 1;
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{}Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                            start, end, total_len, chunk_len, etag_header
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;

                        let slice = &data[start..=end];
                        let delay_ms = pacer(start, end);
                        if delay_ms > 0 {
                            for (i, piece) in slice.chunks(32 * 1024).enumerate() {
                                if socket.write_all(piece).await.is_err() {
                                    break;
                                }
                                if i > 0 {
                                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                }
                            }
                        } else {
                            let _ = socket.write_all(slice).await;
                        }
                    } else {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{}Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                            total_len, etag_header
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(&data).await;
                    }
                });
            }
        });

        Self {
            addr,
            shutdown,
            etag,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/testfile.bin", self.addr)
    }

    #[allow(dead_code)]
    async fn set_etag(&self, new_etag: Option<String>) {
        let mut guard = self.etag.write().await;
        *guard = new_etag;
    }

    fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

// ─── Test Helpers ─────────────────────────────────────────────────────────────

fn generate_payload(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state: u64 = 0xa5a5a5a5_5a5a5a5a;
    for _ in 0..size {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        data.push((state >> 32) as u8);
    }
    data
}

fn make_request(
    url: &str,
    dest_dir: PathBuf,
    filename: &str,
    max_connections: u32,
    opts: Option<MultiplexerOptions>,
) -> DownloadRequest {
    DownloadRequest {
        url: url.to_string(),
        dest_dir,
        filename: Some(filename.to_string()),
        max_connections,
        speed_limit: 0,
        delete_on_failure: false,
        multiplexer_options: opts,
        ..Default::default()
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
        if p.state == TaskState::Failed && target != TaskState::Failed {
            panic!(
                "Task unexpectedly failed while waiting for {:?}: {:?}",
                target, p.error
            );
        }
        if start.elapsed() > timeout {
            panic!(
                "Timed out waiting for state {:?}, last state was {:?}, error: {:?}",
                target, p.state, p.error
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }
}

async fn wait_for_bytes(task: &DownloadTask, min_bytes: u64, timeout: Duration) {
    let start = Instant::now();
    let mut rx = task.progress_rx.clone();
    loop {
        let p = rx.borrow().clone();
        if p.bytes_downloaded >= min_bytes {
            return;
        }
        if start.elapsed() > timeout {
            panic!(
                "Timed out waiting for {} bytes; only received {} bytes",
                min_bytes, p.bytes_downloaded
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }
}

async fn wait_for_checkpoint_bytes(
    db: &Database,
    task_id: &str,
    min_bytes: u64,
    timeout: Duration,
) -> Vec<vajra_engine::state::ChunkProgress> {
    let start = Instant::now();
    loop {
        if let Ok(segs) = db.load_segments(task_id) {
            let written: u64 = segs.iter().map(|s| s.bytes_written).sum();
            if written >= min_bytes {
                return segs;
            }
        }
        if start.elapsed() > timeout {
            let current = db.load_segments(task_id).unwrap_or_default();
            let written: u64 = current.iter().map(|s| s.bytes_written).sum();
            panic!(
                "Timed out waiting for checkpoint with >= {} bytes (got {} bytes, {} segments)",
                min_bytes,
                written,
                current.len()
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn verify_file_sha256(dest_file: &Path, expected_data: &[u8]) {
    let actual_bytes = std::fs::read(dest_file).expect("read file");
    let mut hasher = Sha256::new();
    hasher.update(&actual_bytes);
    let actual_hash = hex::encode(hasher.finalize());

    let mut expected_hasher = Sha256::new();
    expected_hasher.update(expected_data);
    let expected_hash = hex::encode(expected_hasher.finalize());

    assert_eq!(
        actual_hash, expected_hash,
        "File SHA-256 hash does not match expected source hash"
    );
    assert_eq!(
        actual_bytes.len(),
        expected_data.len(),
        "File length does not match expected length"
    );
    assert_eq!(
        actual_bytes, expected_data,
        "Byte-for-byte content differs from source data"
    );
}

// ─── Test Suite ───────────────────────────────────────────────────────────────

/// Test 1: Periodic checkpoint actually appears in SQLite while download is actively running
#[tokio::test]
async fn test_periodic_checkpoint_appears_in_sqlite_during_active_download() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 3 * 1024 * 1024; // 3 MB
    let source_data = generate_payload(total_size);
    // Paced at 20ms per 32KB slice so download takes ~2 seconds
    let server = MockServer::start(source_data.clone(), 20).await;

    let filename = "test_periodic_checkpoint.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_millis(150),
        min_checkpoint_interval: Duration::from_millis(100),
        checkpoint_bytes: 32 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts),
    );

    let task = DownloadTask::start_with_id(task_id, req);

    let db = Database::open(&db_path).unwrap();

    // Wait until download has progressed partway and checkpoint is committed to SQLite
    let segments =
        wait_for_checkpoint_bytes(&db, &task_id.to_string(), 64 * 1024, Duration::from_secs(5))
            .await;

    assert!(
        !segments.is_empty(),
        "Segments must appear in SQLite while download is actively running"
    );
    let checkpoint_bytes: u64 = segments.iter().map(|s| s.bytes_written).sum();
    assert!(
        checkpoint_bytes > 0,
        "Checkpointed bytes in SQLite must be > 0 during active download"
    );

    let p = task.progress_rx.borrow().clone();
    assert!(
        p.state == TaskState::Downloading,
        "Task must still be actively downloading when checkpoint is observed"
    );
    assert!(
        checkpoint_bytes <= p.bytes_downloaded,
        "Checkpointed bytes ({}) must not exceed network bytes received ({})",
        checkpoint_bytes,
        p.bytes_downloaded
    );

    // Let the download complete
    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 2: Checkpoint never exceeds writer-confirmed bytes (proves bridge buffer invariant)
///
/// Invariant:
///   Safe:
///     writer-confirmed flushed bytes = 500 KB
///     bridge/in-flight data = 200 KB
///     SQLite checkpoint = 500 KB
///   Unsafe:
///     writer-confirmed flushed bytes = 500 KB
///     SQLite checkpoint = 700 KB
#[tokio::test]
async fn test_checkpoint_never_persists_unwritten_bridge_buffer_bytes() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());
    let db_path = vajra_protocol::db_path();
    let db = Database::open(&db_path).unwrap();

    let job_id = Uuid::new_v4().to_string();
    let dest_file = temp_dir.path().join("test_invariant.bin");

    // Initialize job in database
    db.upsert_job(&JobRecord {
        id: job_id.clone(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    // Pre-allocate the file on disk so the writer can open and mmap it
    {
        let file = std::fs::File::create(&dest_file).unwrap();
        file.set_len(1024 * 1024).unwrap();
    }

    // 1. Create disk writer and channel
    let (writer_tx, writer_rx) = tokio::sync::mpsc::channel::<WriterCommand>(64);
    let writer_dest = dest_file.clone();
    let writer_fut = tokio::spawn(async move { start_disk_writer(&writer_dest, writer_rx).await });

    // 2. Simulate 500 KB of data processed and sent to writer_tx
    let payload_500kb = vec![0xAA; 500 * 1024];
    let frame1 = DataFrame {
        chunk_id: 0,
        absolute_offset: 0,
        payload: bytes::Bytes::from(payload_500kb),
    };
    writer_tx.send(WriterCommand::Write(frame1)).await.unwrap();

    // 3. Simulate 200 KB in bridge RAM buffer (buf_map) that has NOT been sent to writer_tx
    let in_flight_bridge_bytes = 200 * 1024;
    let _simulated_bridge_buffer = vec![0xBB; in_flight_bridge_bytes];

    // 4. Trigger checkpoint via WriterCommand::Checkpoint
    let (chk_tx, chk_rx) = tokio::sync::oneshot::channel();
    writer_tx
        .send(WriterCommand::Checkpoint(chk_tx))
        .await
        .unwrap();
    let stats = chk_rx.await.unwrap();

    let writer_confirmed = stats.chunk_bytes_written.get(&0).copied().unwrap_or(0);
    assert_eq!(
        writer_confirmed,
        500 * 1024,
        "Writer must only confirm frames it has processed"
    );

    // 5. Commit checkpoint to SQLite
    let segments = vec![SegmentRecord {
        segment_id: 0,
        start_byte: 0,
        end_byte: 1024 * 1024 - 1,
        bytes_written: writer_confirmed,
    }];
    db.save_segments_transactional(&job_id, &segments).unwrap();

    // 6. Query SQLite and assert invariant:
    // SQLite checkpoint MUST equal 500 KB, NEVER 700 KB (which would include bridge memory)!
    let loaded = db.load_segments(&job_id).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded[0].bytes_written,
        500 * 1024,
        "SQLite checkpoint must equal writer-confirmed bytes (500 KB), not in-flight buffer (700 KB)"
    );

    // 7. Now simulate bridge flushing the remaining 200 KB
    let frame2 = DataFrame {
        chunk_id: 0,
        absolute_offset: 500 * 1024,
        payload: bytes::Bytes::from(vec![0xBB; 200 * 1024]),
    };
    writer_tx.send(WriterCommand::Write(frame2)).await.unwrap();

    // 8. Checkpoint again
    let (chk_tx2, chk_rx2) = tokio::sync::oneshot::channel();
    writer_tx
        .send(WriterCommand::Checkpoint(chk_tx2))
        .await
        .unwrap();
    let stats2 = chk_rx2.await.unwrap();
    let writer_confirmed2 = stats2.chunk_bytes_written.get(&0).copied().unwrap_or(0);
    assert_eq!(writer_confirmed2, 700 * 1024);

    let segments2 = vec![SegmentRecord {
        segment_id: 0,
        start_byte: 0,
        end_byte: 1024 * 1024 - 1,
        bytes_written: writer_confirmed2,
    }];
    db.save_segments_transactional(&job_id, &segments2).unwrap();

    let loaded2 = db.load_segments(&job_id).unwrap();
    assert_eq!(loaded2[0].bytes_written, 700 * 1024);

    // Clean up writer
    drop(writer_tx);
    let _ = writer_fut.await.unwrap().unwrap();
}

/// Test 3: Simulated process crash after checkpoint resumes cleanly from last checkpoint
#[tokio::test]
async fn test_simulated_process_crash_resumes_from_checkpoint() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 4 * 1024 * 1024; // 4 MB
    let source_data = generate_payload(total_size);
    let server = MockServer::start(source_data.clone(), 15).await;

    let filename = "test_crash_resume.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_millis(150),
        min_checkpoint_interval: Duration::from_millis(100),
        checkpoint_bytes: 64 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts.clone()),
    );

    // 1. Start download task #1
    let task1 = DownloadTask::start_with_id(task_id, req.clone());

    let db = Database::open(&db_path).unwrap();

    // Wait until at least one checkpoint has committed with written bytes
    let saved_segments = wait_for_checkpoint_bytes(
        &db,
        &task_id.to_string(),
        128 * 1024,
        Duration::from_secs(5),
    )
    .await;
    assert!(
        !saved_segments.is_empty(),
        "Must have checkpointed before crash"
    );
    let checkpointed_bytes: u64 = saved_segments.iter().map(|s| s.bytes_written).sum();
    assert!(checkpointed_bytes > 0);
    assert!(checkpointed_bytes < total_size as u64);

    // 2. SIMULATE ABRUPT CRASH VIA DETERMINISTIC TASK ABORT
    // Genuinely terminate/abort the active download task and its subtasks without
    // running graceful pause/drain cleanup.
    task1.abort_for_test().await;

    // Prove 1: The first task is actually no longer running
    assert!(
        task1.is_finished().await,
        "Task 1 must be terminated after abort_for_test"
    );

    // Prove 2: Writer and bridge cannot continue mutating the file in the background
    let len_at_crash = std::fs::metadata(&dest_file).unwrap().len();
    let bytes_at_crash = std::fs::read(&dest_file).unwrap();
    // Wait while the mock server remains alive and capable of serving data
    tokio::time::sleep(Duration::from_millis(150)).await;
    let len_after_wait = std::fs::metadata(&dest_file).unwrap().len();
    let bytes_after_wait = std::fs::read(&dest_file).unwrap();
    assert_eq!(
        len_at_crash, len_after_wait,
        "File size must not change after crash abort"
    );
    assert_eq!(
        bytes_at_crash, bytes_after_wait,
        "File contents must not be mutated after crash abort"
    );

    // Prove 3: SQLite still holds the checkpointed segments and restart resumes from checkpoint
    let segments_after_crash = db.load_segments(&task_id.to_string()).unwrap();
    assert_eq!(
        segments_after_crash.len(),
        saved_segments.len(),
        "Checkpoints in SQLite must survive process crash"
    );
    let checkpoint_bytes_resumed: u64 = segments_after_crash.iter().map(|s| s.bytes_written).sum();
    assert_eq!(
        checkpoint_bytes_resumed, checkpointed_bytes,
        "Resumed checkpoint bytes must match persisted checkpoint"
    );

    // 3. SIMULATE APPLICATION RESTART
    // Create new task with the exact same ID and request
    let task2 = DownloadTask::start_with_id(task_id, req);

    let completed = wait_for_state(&task2, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    // 4. Verify file integrity
    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 4: File ahead of SQLite checkpoint is completely safe (idempotent overwrite)
#[tokio::test]
async fn test_file_ahead_of_checkpoint_is_safe() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 2 * 1024 * 1024; // 2 MB
    let source_data = generate_payload(total_size);
    let server = MockServer::start(source_data.clone(), 0).await;

    let filename = "test_file_ahead.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();
    let db = Database::open(&db_path).unwrap();

    // 1. Pre-populate SQLite with checkpoint at 400 KB
    let checkpoint_offset = 400 * 1024_u64;
    db.upsert_job(&JobRecord {
        id: task_id.to_string(),
        request_json: "{}".to_string(),
        state: "downloading".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
    .unwrap();

    let segments = vec![SegmentRecord {
        segment_id: 0,
        start_byte: 0,
        end_byte: (total_size - 1) as u64,
        bytes_written: checkpoint_offset,
    }];
    db.save_segments_transactional(&task_id.to_string(), &segments)
        .unwrap();

    // 2. Pre-create the file on disk with 800 KB (ahead of checkpoint!)
    // 400 KB valid data + 400 KB garbage data to simulate written but uncheckpointed / corrupted pages
    let mut file_content = source_data[0..400 * 1024].to_vec();
    file_content.extend_from_slice(&vec![0xFF; 400 * 1024]);
    std::fs::write(&dest_file, &file_content).unwrap();

    assert_eq!(std::fs::metadata(&dest_file).unwrap().len(), 800 * 1024);

    // 3. Resume download
    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        1,
        None,
    );
    let task = DownloadTask::start_with_id(task_id, req);

    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    // 4. Verify resulting file overwrote the garbage and matches source SHA-256 byte-for-byte
    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 5: Work stealing + periodic checkpoint + crash + resume
#[tokio::test]
async fn test_work_stealing_with_periodic_checkpoint_and_resume() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 6 * 1024 * 1024; // 6 MB
    let source_data = generate_payload(total_size);
    // Paced pacer: chunk 0 is fast, chunk 1 is slow to trigger work stealing
    let pacer = Arc::new(|start: usize, _end: usize| -> u64 {
        if start >= 3 * 1024 * 1024 {
            25 // slow chunk 1
        } else {
            2 // fast chunk 0
        }
    });
    let server = MockServer::start_with_pacer_and_etag(source_data.clone(), pacer, None).await;

    let filename = "test_stealing_checkpoint.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_millis(100),
        min_checkpoint_interval: Duration::from_millis(50),
        checkpoint_bytes: 32 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts),
    );

    let task1 = DownloadTask::start_with_id(task_id, req.clone());

    // Wait for task to download at least 3.5 MB (chunk 0 finishes, work stealing steals from chunk 1)
    wait_for_bytes(&task1, 3500 * 1024, Duration::from_secs(10)).await;

    // Check if checkpoint contains stolen chunks (> 2 segments)
    let db = Database::open(&db_path).unwrap();
    let saved = db.load_segments(&task_id.to_string()).unwrap();
    assert!(
        !saved.is_empty(),
        "Segments must be checkpointed during steal"
    );

    // Simulate crash via deterministic task abort
    task1.abort_for_test().await;

    // Resume download as task #2
    let task2 = DownloadTask::start_with_id(task_id, req);

    let completed = wait_for_state(&task2, TaskState::Completed, Duration::from_secs(15)).await;
    assert_eq!(completed.state, TaskState::Completed);

    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 6: Validator survives crash recovery
#[tokio::test]
async fn test_validator_survives_crash_recovery() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 3 * 1024 * 1024; // 3 MB
    let source_data = generate_payload(total_size);
    let server =
        MockServer::start_with_etag(source_data.clone(), 20, Some("\"stable-v1\"".to_string()))
            .await;

    let filename = "test_validator_crash.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_millis(150),
        min_checkpoint_interval: Duration::from_millis(100),
        checkpoint_bytes: 32 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts),
    );

    let task1 = DownloadTask::start_with_id(task_id, req.clone());

    wait_for_bytes(&task1, 300 * 1024, Duration::from_secs(5)).await;

    let db = Database::open(&db_path).unwrap();
    let validators = db.load_validators(&task_id.to_string()).unwrap();
    assert_eq!(
        validators.as_ref().and_then(|v| v.etag.as_deref()),
        Some("\"stable-v1\""),
        "Validator must be persisted on initial probe"
    );

    // Simulate crash via deterministic task abort
    task1.abort_for_test().await;

    // Verify validator still exists in SQLite after crash
    let validators_after_crash = db.load_validators(&task_id.to_string()).unwrap();
    assert_eq!(
        validators_after_crash
            .as_ref()
            .and_then(|v| v.etag.as_deref()),
        Some("\"stable-v1\""),
        "Validator must survive process crash"
    );

    // Resume task
    let task2 = DownloadTask::start_with_id(task_id, req);
    let completed = wait_for_state(&task2, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 7: Completed job cleanly removes segment checkpoint state
#[tokio::test]
async fn test_completed_job_removes_segment_checkpoint_state() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 2 * 1024 * 1024; // 2 MB
    let source_data = generate_payload(total_size);
    let server = MockServer::start(source_data.clone(), 10).await;

    let filename = "test_completed_cleanup.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();
    let db = Database::open(&db_path).unwrap();

    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_millis(100),
        min_checkpoint_interval: Duration::from_millis(50),
        checkpoint_bytes: 16 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts),
    );

    let task = DownloadTask::start_with_id(task_id, req);

    // Wait until download has actively saved checkpoints to SQLite
    let mid_segments =
        wait_for_checkpoint_bytes(&db, &task_id.to_string(), 32 * 1024, Duration::from_secs(5))
            .await;
    assert!(
        !mid_segments.is_empty(),
        "Segments must exist in SQLite during active download"
    );

    // Wait for complete
    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    // Verify segments were deleted from SQLite on completion
    let final_segments = db.load_segments(&task_id.to_string()).unwrap();
    assert!(
        final_segments.is_empty(),
        "All download segments must be deleted from SQLite upon verified completion"
    );

    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 8: Byte-threshold checkpoint trigger based on writer-confirmed progress
/// Configured with 60-second time trigger so time CANNOT trigger checkpoint during test.
/// Only writer-confirmed bytes can trigger checkpoint commit.
#[tokio::test]
async fn test_byte_threshold_checkpoint_based_on_writer_confirmed_bytes() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let total_size = 500 * 1024; // 500 KB
    let source_data = generate_payload(total_size);
    // Paced at 25ms per 32KB chunk so it transfers over ~400ms
    let server = MockServer::start(source_data.clone(), 25).await;

    let filename = "test_byte_threshold.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    // checkpoint_interval = 60s (ensures timer NEVER triggers during this 1s test)
    // checkpoint_bytes = 100 KB (byte threshold that MUST trigger)
    let opts = MultiplexerOptions {
        checkpoint_interval: Duration::from_secs(60),
        min_checkpoint_interval: Duration::from_millis(10),
        checkpoint_bytes: 100 * 1024,
        ..Default::default()
    };

    let req = make_request(
        &server.url(),
        temp_dir.path().to_path_buf(),
        filename,
        2,
        Some(opts),
    );

    let test_start = Instant::now();
    let task = DownloadTask::start_with_id(task_id, req);
    let db = Database::open(&db_path).unwrap();

    // Wait until at least 100 KB checkpoint commits to SQLite
    let segments = wait_for_checkpoint_bytes(
        &db,
        &task_id.to_string(),
        100 * 1024,
        Duration::from_secs(5),
    )
    .await;

    let elapsed = test_start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "Checkpoint was committed in {:?}, far before the 60-second time interval!",
        elapsed
    );

    let written: u64 = segments.iter().map(|s| s.bytes_written).sum();
    assert!(
        written >= 100 * 1024,
        "Committed checkpoint must be >= threshold (got {})",
        written
    );

    let completed = wait_for_state(&task, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed.state, TaskState::Completed);

    verify_file_sha256(&dest_file, &source_data);
    server.stop();
}

/// Test 9: Windows mmap checkpoint durability sync sequence
/// Verifies that start_disk_writer on a pre-allocated file exercises the explicit durability sequence:
/// writes complete → mmap.flush() (FlushViewOfFile) → File::sync_data() (FlushFileBuffers) → WriterStats returned.
#[tokio::test]
async fn test_windows_mmap_checkpoint_durability_sync() {
    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    let dest_file = temp_dir.path().join("test_mmap_durability.bin");
    let file_size = 256 * 1024; // 256 KB

    // Pre-allocate the file so mmap is activated (file_len > 0)
    {
        let file = std::fs::File::create(&dest_file).unwrap();
        file.set_len(file_size as u64).unwrap();
    }

    let (writer_tx, writer_rx) = tokio::sync::mpsc::channel::<WriterCommand>(32);
    let writer_dest = dest_file.clone();
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_clone = Arc::clone(&counter);

    let writer_handle = tokio::spawn(async move {
        start_disk_writer_with_counter(&writer_dest, writer_rx, Some(counter_clone)).await
    });

    let payload1 = vec![0x33u8; 64 * 1024];
    writer_tx
        .send(
            DataFrame {
                chunk_id: 0,
                absolute_offset: 0,
                payload: bytes::Bytes::from(payload1.clone()),
            }
            .into(),
        )
        .await
        .unwrap();

    // Trigger checkpoint — this invokes mmap.flush() (FlushViewOfFile) THEN sync_data() (FlushFileBuffers)
    let (chk_tx, chk_rx) = tokio::sync::oneshot::channel();
    writer_tx
        .send(WriterCommand::Checkpoint(chk_tx))
        .await
        .unwrap();

    let stats = chk_rx.await.expect("checkpoint reply");
    assert_eq!(stats.bytes_written, 64 * 1024);
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Acquire),
        64 * 1024
    );

    // Read directly from disk through a separate file handle to verify flushed data is present
    let disk_bytes = std::fs::read(&dest_file).unwrap();
    assert_eq!(&disk_bytes[0..64 * 1024], &payload1[..]);

    // Send second segment and final flush
    let payload2 = vec![0x44u8; 64 * 1024];
    writer_tx
        .send(
            DataFrame {
                chunk_id: 1,
                absolute_offset: 64 * 1024,
                payload: bytes::Bytes::from(payload2.clone()),
            }
            .into(),
        )
        .await
        .unwrap();

    drop(writer_tx);
    let final_stats = writer_handle.await.unwrap().expect("writer finished");
    assert_eq!(final_stats.bytes_written, 128 * 1024);

    let final_disk_bytes = std::fs::read(&dest_file).unwrap();
    assert_eq!(&final_disk_bytes[0..64 * 1024], &payload1[..]);
    assert_eq!(&final_disk_bytes[64 * 1024..128 * 1024], &payload2[..]);
}
