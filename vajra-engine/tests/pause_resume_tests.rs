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
use vajra_engine::download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskState};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ─── Mock HTTP Server supporting byte ranges and flexible pacing ──────────────

type PacerFn = Arc<dyn Fn(usize, usize) -> u64 + Send + Sync>;

struct MockServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
}

impl MockServer {
    async fn start(data: Vec<u8>, chunk_delay_ms: u64) -> Self {
        Self::start_with_pacer(data, Arc::new(move |_, _| chunk_delay_ms)).await
    }

    async fn start_with_pacer(data: Vec<u8>, pacer: PacerFn) -> Self {
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
                let pacer = Arc::clone(&pacer);

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

                    // Parse Range header if present (e.g. "Range: bytes=0-1048575")
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

                        let slice = &data[start..=end];
                        let delay_ms = pacer(start, end);
                        if delay_ms > 0 {
                            for (i, piece) in slice.chunks(16 * 1024).enumerate() {
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
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                            total_len
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(&data).await;
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

// ─── Helper Functions ─────────────────────────────────────────────────────────

fn generate_deterministic_payload(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state: u64 = 0x12345678_9abcdef0;
    for _ in 0..size {
        // Simple 64-bit LCG for deterministic byte stream
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
                "Timed out waiting for state {:?}, last state was {:?}",
                target, p.state
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

async fn wait_for_chunk_done(task: &DownloadTask, chunk_id: usize, timeout: Duration) {
    let start = Instant::now();
    let mut rx = task.progress_rx.clone();
    loop {
        let p = rx.borrow().clone();
        if let Some(seg) = p.segments.iter().find(|s| s.id == chunk_id) {
            if seg.status == vajra_protocol::DownloadStatus::Completed {
                return;
            }
        }
        if start.elapsed() > timeout {
            panic!("Timed out waiting for chunk {} to complete", chunk_id);
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }
}

fn verify_segments_on_disk(
    dest_file: &Path,
    source_data: &[u8],
    db: &vajra_engine::db::Database,
    task_id: &Uuid,
) {
    let segments = db
        .load_segments(&task_id.to_string())
        .expect("load segments");
    assert!(!segments.is_empty(), "Segments must be persisted in SQLite");

    let file_bytes = std::fs::read(dest_file).expect("read destination file");

    for seg in segments {
        let start = seg.start_byte.expect("start_byte") as usize;
        let written = seg.bytes_written as usize;
        if written > 0 {
            assert!(
                start + written <= file_bytes.len(),
                "Written range {}..{} extends past file len {}",
                start,
                start + written,
                file_bytes.len()
            );
            let disk_slice = &file_bytes[start..start + written];
            let source_slice = &source_data[start..start + written];
            assert_eq!(
                disk_slice,
                source_slice,
                "Data mismatch in segment {} at range {}..{}",
                seg.chunk_id,
                start,
                start + written
            );
        }
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

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Test 1-4:
/// 1. Download → Pause
/// 2. Verify persisted bytes exactly match bytes actually written (verify ranges on disk)
/// 3. Resume
/// 4. Verify resulting file is byte-for-byte correct via SHA-256
#[tokio::test]
async fn test_download_pause_verify_persisted_bytes_and_resume_integrity() {
    let total_size = 4 * 1024 * 1024; // 4 MB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data.clone(), 6).await;

    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());
    let filename = "test_pause_resume.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 4);

    // 1. Start download
    let task = DownloadTask::start_with_id(task_id, req.clone());

    // Wait until at least 256 KB downloaded
    wait_for_bytes(&task, 256 * 1024, Duration::from_secs(5)).await;

    // Pause
    task.pause().await;
    let paused_progress = wait_for_state(&task, TaskState::Paused, Duration::from_secs(5)).await;
    assert_eq!(paused_progress.state, TaskState::Paused);

    // 2. Open isolated DB and verify persisted segments match disk ranges byte-for-byte
    let db = vajra_engine::db::Database::open(&db_path).unwrap();
    verify_segments_on_disk(&dest_file, &source_data, &db, &task_id);

    // 3. Resume download
    let resume_task = DownloadTask::start_with_id(task_id, req.clone());
    let completed_progress =
        wait_for_state(&resume_task, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(completed_progress.state, TaskState::Completed);

    // 4. Verify resulting file is byte-for-byte correct
    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}

/// Test 5-6:
/// 5. Pause → Resume → Pause → Resume (Repeated cycles)
/// 6. Verify repeated pause/resume does NOT skip data and preserves exact offsets
#[tokio::test]
async fn test_repeated_pause_resume_does_not_skip_data() {
    let total_size = 6 * 1024 * 1024; // 6 MB
    let source_data = generate_deterministic_payload(total_size);
    let server = MockServer::start(source_data.clone(), 4).await;

    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());
    let filename = "test_repeated_pause_resume.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 4);
    let db = vajra_engine::db::Database::open(&db_path).unwrap();

    // ── Cycle 1: Download → Pause ──────────────────────────────────────────
    let task1 = DownloadTask::start_with_id(task_id, req.clone());
    wait_for_bytes(&task1, 256 * 1024, Duration::from_secs(5)).await;
    task1.pause().await;
    wait_for_state(&task1, TaskState::Paused, Duration::from_secs(5)).await;

    // Verify Cycle 1 written ranges on disk
    verify_segments_on_disk(&dest_file, &source_data, &db, &task_id);

    // ── Cycle 2: Resume → Pause ──────────────────────────────────────────
    let task2 = DownloadTask::start_with_id(task_id, req.clone());
    wait_for_bytes(&task2, 1024 * 1024, Duration::from_secs(5)).await;
    task2.pause().await;
    wait_for_state(&task2, TaskState::Paused, Duration::from_secs(5)).await;

    // Verify Cycle 2 written ranges on disk - ensures original_start_byte was not shifted
    verify_segments_on_disk(&dest_file, &source_data, &db, &task_id);

    // ── Cycle 3: Resume → Pause ──────────────────────────────────────────
    let task3 = DownloadTask::start_with_id(task_id, req.clone());
    wait_for_bytes(&task3, 2 * 1024 * 1024, Duration::from_secs(5)).await;
    task3.pause().await;
    wait_for_state(&task3, TaskState::Paused, Duration::from_secs(5)).await;

    // Verify Cycle 3 written ranges on disk
    verify_segments_on_disk(&dest_file, &source_data, &db, &task_id);

    // ── Final: Resume to completion ──────────────────────────────────────
    let task4 = DownloadTask::start_with_id(task_id, req.clone());
    let final_progress =
        wait_for_state(&task4, TaskState::Completed, Duration::from_secs(10)).await;
    assert_eq!(final_progress.state, TaskState::Completed);

    // Verify byte-for-byte correctness of the entire file
    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}

/// Test 7-8:
/// 7. Pause while buffers contain unwritten data
/// 8. Pause while writer channel contains queued frames
/// Verifies graceful drain ensures zero data loss and resulting file is byte-for-byte correct
#[tokio::test]
async fn test_pause_while_buffers_and_channels_contain_queued_data() {
    let total_size = 4 * 1024 * 1024; // 4 MB
    let source_data = generate_deterministic_payload(total_size);
    // Introduce 5ms delay per 16KB chunk to ensure network frames and writer queue are continuously active
    let server = MockServer::start(source_data.clone(), 5).await;

    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());
    let filename = "test_buffered_pause.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 4);
    let db = vajra_engine::db::Database::open(&db_path).unwrap();

    // Start download
    let task = DownloadTask::start_with_id(task_id, req.clone());

    // Wait until download is actively streaming data into RAM buffers
    wait_for_bytes(&task, 128 * 1024, Duration::from_secs(5)).await;

    // Trigger pause while streaming is in progress (RAM buffers & writer channel full)
    task.pause().await;
    let paused_progress = wait_for_state(&task, TaskState::Paused, Duration::from_secs(8)).await;
    assert_eq!(paused_progress.state, TaskState::Paused);

    // Verify all drained and committed data matches source data exactly
    verify_segments_on_disk(&dest_file, &source_data, &db, &task_id);

    // Resume to completion
    let resume_task = DownloadTask::start_with_id(task_id, req.clone());
    let completed_progress =
        wait_for_state(&resume_task, TaskState::Completed, Duration::from_secs(15)).await;
    assert_eq!(completed_progress.state, TaskState::Completed);

    // Verify byte-for-byte SHA-256 equivalence
    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}

/// Test 9-10:
/// Work-stealing regression test:
/// 1. Create a multi-chunk download with IDs such as 0,1,2,3.
/// 2. Make at least one chunk complete before a resume.
/// 3. Resume so the active registry has non-contiguous IDs.
/// 4. Force work stealing.
/// 5. Verify the new stolen chunk receives a unique ID.
/// 6. Pause after stealing.
/// 7. Verify SQLite contains distinct segment rows.
/// 8. Resume.
/// 9. Complete the download.
/// 10. Verify the final file is byte-for-byte identical to the original source using SHA-256.
#[tokio::test]
async fn test_work_stealing_non_contiguous_ids_and_resume_integrity() {
    let total_size = 16 * 1024 * 1024; // 16 MB (4 chunks of 4MB each: 0, 1, 2, 3)
    let source_data = generate_deterministic_payload(total_size);

    // Dynamic pacer mode:
    // Mode 0: Chunk 0 (0..4MB) downloads fast (0ms delay); Chunks 1, 2, 3 are slow (50ms/16KB)
    // Mode 1: Chunk 1 (4MB..8MB) downloads fast (0ms delay); Chunks 2, 3 are slow (50ms/16KB)
    // Mode 2: All chunks download fast (0ms delay)
    let pacer_mode = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let pacer_mode_clone = pacer_mode.clone();

    let server = MockServer::start_with_pacer(
        source_data.clone(),
        Arc::new(
            move |start, _end| match pacer_mode_clone.load(Ordering::Relaxed) {
                0 => {
                    if start < 4 * 1024 * 1024 {
                        0
                    } else {
                        50
                    }
                }
                1 => {
                    if (4 * 1024 * 1024..8 * 1024 * 1024).contains(&start) {
                        1
                    } else {
                        50
                    }
                }
                _ => 0,
            },
        ),
    )
    .await;

    let _lock = TEST_MUTEX.lock().await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());
    let filename = "test_work_stealing_regression.bin";
    let dest_file = temp_dir.path().join(filename);
    let task_id = Uuid::new_v4();
    let db_path = vajra_protocol::db_path();

    let req = make_request(&server.url(), temp_dir.path().to_path_buf(), filename, 4);
    let db = vajra_engine::db::Database::open(&db_path).unwrap();

    // ── Phase 1: Download until Chunk 0 completes, then pause ─────────────
    let task1 = DownloadTask::start_with_id(task_id, req.clone());

    // Wait until Chunk 0 is fully completed
    wait_for_chunk_done(&task1, 0, Duration::from_secs(10)).await;

    task1.pause().await;
    let paused_p1 = wait_for_state(&task1, TaskState::Paused, Duration::from_secs(5)).await;
    assert_eq!(paused_p1.state, TaskState::Paused);

    // Verify in SQLite: Chunk 0 completed, Chunks 1, 2, 3 partially written
    let segments_p1 = db.load_segments(&task_id.to_string()).unwrap();
    let seg0 = segments_p1
        .iter()
        .find(|s| s.chunk_id == 0)
        .expect("chunk 0 segment must exist");
    assert_eq!(
        seg0.bytes_written,
        4 * 1024 * 1024,
        "Chunk 0 should be fully completed"
    );

    // ── Phase 2: Resume with non-contiguous IDs; Chunk 1 completes and steals work ──
    pacer_mode.store(1, Ordering::Relaxed);

    let task2 = DownloadTask::start_with_id(task_id, req.clone());

    // Wait for work stealing to happen:
    // When Chunk 1 finishes, it steals remaining work from Chunk 2 or 3.
    // The newly stolen chunk MUST receive a unique ID >= 4 (NOT colliding with chunk 3)!
    let mut rx = task2.progress_rx.clone();
    let start_wait = Instant::now();
    let mut stolen_id: Option<usize> = None;

    while start_wait.elapsed() < Duration::from_secs(10) {
        let p = rx.borrow().clone();
        if let Some(stolen_seg) = p.segments.iter().find(|s| s.id >= 4) {
            stolen_id = Some(stolen_seg.id);
            break;
        }
        let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
    }

    let stolen_chunk_id = stolen_id
        .expect("Work stealing must have triggered and created a new segment with unique ID >= 4");
    assert_eq!(
        stolen_chunk_id, 4,
        "First stolen chunk must receive ID 4, avoiding collision with existing chunk 3"
    );

    // Pause after stealing to verify SQLite persistence of both donor and stolen chunk
    task2.pause().await;
    let paused_p2 = wait_for_state(&task2, TaskState::Paused, Duration::from_secs(5)).await;
    assert_eq!(paused_p2.state, TaskState::Paused);

    // Verify SQLite contains distinct segment rows (no duplicate IDs, segment 3 not overwritten)
    let segments_p2 = db.load_segments(&task_id.to_string()).unwrap();
    assert!(
        segments_p2.len() >= 5,
        "SQLite must contain at least 5 distinct segment rows (found {})",
        segments_p2.len()
    );

    let mut ids: Vec<usize> = segments_p2.iter().map(|s| s.chunk_id).collect();
    ids.sort();
    let original_len = ids.len();
    ids.dedup();
    assert_eq!(
        ids.len(),
        original_len,
        "Segment IDs in SQLite must be strictly unique"
    );
    assert!(
        ids.contains(&3) && ids.contains(&4),
        "SQLite must contain both existing chunk 3 and newly stolen chunk 4 without collision"
    );

    // ── Phase 3: Resume to completion and verify final byte-for-byte SHA-256 ──
    pacer_mode.store(2, Ordering::Relaxed);

    let task3 = DownloadTask::start_with_id(task_id, req.clone());
    let final_progress =
        wait_for_state(&task3, TaskState::Completed, Duration::from_secs(15)).await;
    assert_eq!(final_progress.state, TaskState::Completed);

    verify_file_sha256(&dest_file, &source_data);

    server.stop();
}
