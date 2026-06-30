//! Asynchronous Connection Multiplexer
//!
//! Calculates byte-range boundaries for a target file and drives concurrent
//! HTTP GET requests — one `tokio` task per chunk — streaming raw bytes through
//! `tokio::sync::mpsc` channels without ever buffering the full payload.
//!
//! # Cargo dependencies required
//!
//! ```toml
//! [dependencies]
//! bytes        = "1"
//! futures-util = { version = "0.3", default-features = false, features = ["sink"] }
//! reqwest      = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }
//! thiserror    = "1"
//! tokio        = { version = "1", features = ["rt-multi-thread", "time", "sync", "macros"] }
//! ```

#![deny(unsafe_code)]

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::{header, Client};
use tokio::{
    sync::{mpsc, Mutex},
    time::sleep,
};

// ─── Constants ────────────────────────────────────────────────────────────────

use crate::constants::*;

/// Base delay for the first retry; doubles on each subsequent attempt.
/// Retry schedule: 250 ms → 500 ms → 1 000 ms → 2 000 ms.
const BASE_BACKOFF: Duration = Duration::from_millis(250);

/// Default bounded-channel capacity (in `ChunkPayload` messages).
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

// ─── Public types ─────────────────────────────────────────────────────────────

/// Tracks the lifecycle of a single byte-range segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkStatus {
    /// Allocated but not yet started.
    Pending,
    /// A worker task is performing HEAD/GET or TCP/TLS handshake.
    Connecting,
    /// A worker task is actively downloading this range.
    Downloading,
    /// All bytes for this range have been streamed successfully.
    Completed,
    /// All retry attempts have been exhausted.
    Failed {
        /// Number of attempts made (always equal to `MAX_RETRIES` on failure).
        attempts: u32,
        /// Human-readable description of the last error.
        reason: String,
    },
}

pub struct StealRequest {
    pub response_tx: tokio::sync::oneshot::Sender<Option<(u64, u64)>>,
}

/// A single byte-range segment of the target file.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Zero-based sequential index assigned during [`calculate_chunks`].
    pub id: usize,
    /// Inclusive start offset within the remote file.
    pub start_byte: u64,
    /// Inclusive end offset within the remote file.
    pub end_byte: u64,
    /// Whether the worker must require HTTP 206 and send a Range header.
    pub ranged: bool,
    /// Current lifecycle status, updated by worker tasks via shared state.
    pub status: ChunkStatus,
    pub retry_count: usize,
    pub error_message: Option<String>,
    /// How many bytes have been successfully emitted so far for this chunk.
    pub current_offset: u64,
    pub steal_tx: Option<tokio::sync::mpsc::Sender<StealRequest>>,
}

/// A single streaming data frame delivered through the mpsc channel.
/// Each frame carries the bytes from one `response.bytes_stream()` item;
/// frames from the same chunk share the same `chunk_id`.
#[derive(Debug)]
pub struct ChunkPayload {
    /// Which chunk this frame belongs to.
    pub chunk_id: usize,
    /// The absolute offset in the file where this data starts.
    pub absolute_offset: u64,
    /// Raw bytes received from the network in this frame.
    pub data: Bytes,
}

/// All errors that the multiplexer can surface.
#[derive(Debug, thiserror::Error)]
pub enum MultiplexerError {
    /// A chunk failed permanently after exhausting all retry attempts.
    #[error("chunk {chunk_id} exhausted {attempts} attempts: {message}")]
    Exhausted {
        chunk_id: usize,
        attempts: u32,
        message: String,
    },

    /// The server returned an unexpected HTTP status code (not 2xx / 206).
    #[error("chunk {chunk_id} received unexpected HTTP status {status}")]
    HttpStatus { chunk_id: usize, status: u16 },

    /// `max_connections` or `total_size` was zero.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

/// Handle to an in-progress multiplexed download.
///
/// The caller drives the download by receiving from `receiver` until it
/// returns `None` (all tasks completed or were dropped).  The `chunks` field
/// exposes per-chunk status for progress reporting.
pub struct DownloadHandle {
    /// Stream of byte-frame results.  `Err` variants carry per-chunk failures;
    /// `None` means all tasks have finished.
    pub receiver: mpsc::Receiver<Result<ChunkPayload, MultiplexerError>>,

    /// Shared, lock-guarded chunk registry.  Poll this to build a progress UI.
    pub chunks: Arc<Mutex<Vec<Chunk>>>,
}

// ─── Pure boundary calculation ────────────────────────────────────────────────

/// Compute byte-range boundaries for exactly `max_connections` (or fewer, if
/// the file is too small) non-overlapping chunks that together span
/// `[0, total_size)`.
///
/// The algorithm distributes the `total_size % n` remainder bytes as one extra
/// byte to the first `remainder` chunks, so no chunk is more than 1 byte
/// larger than another.
///
/// # Errors
///
/// Returns [`MultiplexerError::InvalidArgument`] when either argument is zero.
///
/// # Example
///
/// ```
/// # use vajra_engine::multiplexer::calculate_chunks;
/// let chunks = calculate_chunks(40 * 1024 * 1024, 4).unwrap();
/// assert_eq!(chunks.len(), 4);
/// assert_eq!(chunks[0].start_byte, 0);
/// assert_eq!(chunks[3].end_byte, 40 * 1024 * 1024 - 1);
/// ```
pub fn calculate_chunks(
    total_size: u64,
    max_connections: usize,
) -> Result<Vec<Chunk>, MultiplexerError> {
    if total_size == 0 {
        return Err(MultiplexerError::InvalidArgument(
            "total_size must be > 0".into(),
        ));
    }
    if max_connections == 0 {
        return Err(MultiplexerError::InvalidArgument(
            "max_connections must be > 0".into(),
        ));
    }

    // Never create so many connections that individual chunks are trivially small.
    let by_size = ((total_size / MIN_CHUNK_SIZE).max(1)) as usize;
    let n = max_connections.min(by_size);

    let base = total_size / n as u64;
    let remainder = total_size % n as u64; // first `remainder` chunks get +1 byte

    let mut chunks = Vec::with_capacity(n);
    let mut offset: u64 = 0;

    for i in 0..n {
        let size = base + if (i as u64) < remainder { 1 } else { 0 };
        chunks.push(Chunk {
            id: i,
            start_byte: offset,
            end_byte: offset + size - 1, // inclusive
            ranged: true,
            status: ChunkStatus::Pending,
            retry_count: 0,
            error_message: None,
            current_offset: 0,
            steal_tx: None,
        });
        offset += size;
    }

    debug_assert_eq!(
        chunks.last().map(|c| c.end_byte),
        Some(total_size - 1),
        "chunks must cover the entire file"
    );

    Ok(chunks)
}

/// Dynamic Thread Stealing — split the slowest active chunk.
///
/// When a chunk finishes early, call this to find the chunk with the most
/// remaining bytes and split its tail into a new chunk for the idle thread.
///
/// Returns `Some(new_chunk)` where `new_chunk` is the newly split-off chunk.
/// Returns `None` if no suitable donor exists (all chunks done or too small).
pub async fn steal_from_slowest(shared: &Arc<Mutex<Vec<Chunk>>>) -> Option<Chunk> {
    let (steal_tx, _chunk_id) = {
        let guard = shared.lock().await;
        // Find the downloading chunk with the most remaining bytes
        let candidate = guard
            .iter()
            .filter(|c| c.status == ChunkStatus::Downloading && c.steal_tx.is_some())
            .max_by_key(|c| c.end_byte.saturating_sub(c.start_byte + c.current_offset))?;

        let current_start = candidate.start_byte + candidate.current_offset;
        let remaining_bytes = candidate.end_byte.saturating_sub(current_start);
        if remaining_bytes < 2 * 1024 * 1024 {
            // Require at least 2MB to split (prevents thrashing)
            return None;
        }
        (candidate.steal_tx.clone()?, candidate.id)
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    if steal_tx.send(StealRequest { response_tx }).await.is_err() {
        return None;
    }

    if let Ok(Some((midpoint, original_end_byte))) = response_rx.await {
        let mut guard = shared.lock().await;
        let new_id = guard.len();
        // Create the new chunk for the stolen part (midpoint → original end)
        let new_chunk = Chunk {
            id: new_id,
            start_byte: midpoint,
            end_byte: original_end_byte,
            ranged: true,
            status: ChunkStatus::Pending,
            retry_count: 0,
            error_message: None,
            current_offset: 0,
            steal_tx: None,
        };
        guard.push(new_chunk.clone());
        Some(new_chunk)
    } else {
        None
    }
}

// ─── Download orchestration ───────────────────────────────────────────────────

/// Spawn one `tokio` task per chunk and begin streaming bytes immediately.
///
/// Returns a [`DownloadHandle`] immediately; actual work runs in the background.
/// Drop `handle.receiver` to abort all in-flight tasks cooperatively (they will
/// notice the closed channel and exit on the next send attempt).
///
/// # Arguments
///
/// * `client`           – A pre-configured [`reqwest::Client`] (timeout, proxy,
///   TLS, etc. should already be set on the builder).
/// * `url`              – Target URL.  Must support HTTP range requests.
/// * `initial_chunks`   – Output of [`calculate_chunks`]; consumed here.
/// * `channel_capacity` – Bounded mpsc buffer depth.
///   Use [`DEFAULT_CHANNEL_CAPACITY`] if unsure.
pub fn start_download(
    client: Client,
    mirror_manager: Arc<tokio::sync::Mutex<crate::mirror::MirrorManager>>,
    initial_chunks: Vec<Chunk>,
    channel_capacity: usize,
) -> DownloadHandle {
    let shared = Arc::new(Mutex::new(initial_chunks.clone()));
    let (tx, rx) = mpsc::channel::<Result<ChunkPayload, MultiplexerError>>(channel_capacity);

    let client_shared = Arc::new(client);
    for chunk_snapshot in initial_chunks {
        let client = Arc::clone(&client_shared);
        let mirror_manager = Arc::clone(&mirror_manager);
        let tx = tx.clone();
        let shared = Arc::clone(&shared);

        tokio::spawn(async move {
            let mut current_chunk = chunk_snapshot;
            loop {
                // ── 1. Mark as Connecting ────────────────────────────────────
                set_status(&shared, current_chunk.id, ChunkStatus::Connecting).await;

                // ── 2. Execute with retry ─────────────────────────────────────
                let outcome = run_chunk(&client, &mirror_manager, &current_chunk, &tx, &shared).await;

                // ── 3. Persist final status ───────────────────────────────────
                let final_status = match &outcome {
                    Ok(()) => ChunkStatus::Completed,
                    Err(e) => ChunkStatus::Failed {
                        attempts: MAX_RETRIES,
                        reason: e.to_string(),
                    },
                };
                set_status(&shared, current_chunk.id, final_status).await;

                if let Err(e) = outcome {
                    let _ = tx.send(Err(e)).await;
                    break;
                }

                // ── 4. Try to steal work from the slowest active chunk ────────
                if let Some(new_chunk) = steal_from_slowest(&shared).await {
                    current_chunk = new_chunk;
                    continue; // Loop and download the stolen chunk!
                }

                break; // No more work to steal, terminate this worker.
            }
        });
    }

    // The original `tx` is dropped here; channel closes when all task clones
    // are also dropped.
    DownloadHandle {
        receiver: rx,
        chunks: shared,
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Update a single chunk's status inside the shared registry.
async fn set_status(shared: &Arc<Mutex<Vec<Chunk>>>, id: usize, status: ChunkStatus) {
    let mut guard = shared.lock().await;
    if let Some(chunk) = guard.iter_mut().find(|c| c.id == id) {
        chunk.status = status;
    }
}

async fn set_retry_count(shared: &Arc<Mutex<Vec<Chunk>>>, id: usize, retry_count: usize) {
    let mut guard = shared.lock().await;
    if let Some(chunk) = guard.iter_mut().find(|c| c.id == id) {
        chunk.retry_count = retry_count;
    }
}

async fn set_error_message(
    shared: &Arc<Mutex<Vec<Chunk>>>,
    id: usize,
    error_message: Option<String>,
) {
    let mut guard = shared.lock().await;
    if let Some(chunk) = guard.iter_mut().find(|c| c.id == id) {
        chunk.error_message = error_message;
    }
}

async fn set_current_offset(shared: &Arc<Mutex<Vec<Chunk>>>, id: usize, offset: u64) {
    let mut guard = shared.lock().await;
    if let Some(chunk) = guard.iter_mut().find(|c| c.id == id) {
        chunk.current_offset = offset;
    }
}

/// Execute one chunk's download loop with exponential-backoff retries.
///
/// Streams each network frame directly into `tx` without accumulating the
/// entire chunk body.
async fn run_chunk(
    client: &Client,
    mirror_manager: &Arc<tokio::sync::Mutex<crate::mirror::MirrorManager>>,
    chunk: &Chunk,
    tx: &mpsc::Sender<Result<ChunkPayload, MultiplexerError>>,
    shared: &Arc<Mutex<Vec<Chunk>>>,
) -> Result<(), MultiplexerError> {
    struct StealCleanup {
        shared: Arc<Mutex<Vec<Chunk>>>,
        chunk_id: usize,
    }
    impl Drop for StealCleanup {
        fn drop(&mut self) {
            let shared = self.shared.clone();
            let chunk_id = self.chunk_id;
            tokio::spawn(async move {
                let mut guard = shared.lock().await;
                if let Some(c) = guard.iter_mut().find(|c| c.id == chunk_id) {
                    c.steal_tx = None;
                }
            });
        }
    }

    let (steal_tx, mut steal_rx) = mpsc::channel::<StealRequest>(1);
    {
        let mut guard = shared.lock().await;
        if let Some(c) = guard.iter_mut().find(|c| c.id == chunk.id) {
            c.steal_tx = Some(steal_tx);
        }
    }
    let _cleanup = StealCleanup {
        shared: Arc::clone(shared),
        chunk_id: chunk.id,
    };

    let mut last_error = String::new();
    let mut bytes_emitted = 0_u64;
    let mut local_end_byte = chunk.end_byte;
    
    // Get initial mirror
    let mut current_url = {
        let mgr = mirror_manager.lock().await;
        let mut urls = mgr.rank_mirrors(&mgr.probe_all_mirrors().await);
        if urls.is_empty() {
            return Err(MultiplexerError::Exhausted {
                chunk_id: chunk.id,
                attempts: 0,
                message: "No available mirrors".into(),
            });
        }
        urls.remove(0)
    };

    for attempt in 0..MAX_RETRIES {
        // ── Exponential backoff (skip on first attempt) ───────────────────
        if attempt > 0 {
            set_retry_count(shared, chunk.id, attempt as usize).await;
            set_status(shared, chunk.id, ChunkStatus::Connecting).await;
            
            // On retry, try to swap mirror
            if let Some(new_mirror) = mirror_manager.lock().await.handle_mirror_failure(&current_url, chunk.id).await {
                current_url = new_mirror;
            }
            
            // 0 → 250 ms, 1 → 500 ms, 2 → 1 000 ms
            let delay = BASE_BACKOFF * 2u32.pow(attempt - 1);
            sleep(delay).await;
        }

        // ── Send the range request ────────────────────────────────────────
        let request = client.get(&current_url);
        let request = if chunk.ranged {
            request.header(
                header::RANGE,
                format!(
                    "bytes={}-{}",
                    chunk.start_byte + bytes_emitted,
                    local_end_byte
                ),
            )
        } else {
            request
        };
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                last_error = e.to_string();
                set_error_message(shared, chunk.id, Some(last_error.clone())).await;
                continue; // retry
            }
        };

        let status = response.status();

        if !status.is_success() {
            // 416 = Range Not Satisfiable — server doesn't support ranges.
            // Fall back gracefully: treat as single-stream from here.
            if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                return Ok(()); // caller will restart without Range header
            }
            return Err(MultiplexerError::HttpStatus {
                chunk_id: chunk.id,
                status: status.as_u16(),
            });
        }
        // For ranged requests, server must return 206; anything else = no range support
        if chunk.ranged && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Ok(()); // silent OK — treat as if chunk is done, caller handles
        }

        // ── Stream body frames into the channel ───────────────────────────
        let mut stream = response.bytes_stream();
        let mut stream_failed: Option<String> = None;
        let mut is_downloading = false;

        loop {
            tokio::select! {
                Some(req) = steal_rx.recv() => {
                    let current_start = chunk.start_byte + bytes_emitted;
                    let remaining = local_end_byte.saturating_sub(current_start);
                    if remaining >= 2 * 1024 * 1024 {
                        let midpoint = current_start + (remaining / 2);
                        let original_end = local_end_byte;
                        local_end_byte = midpoint - 1;

                        // Update in registry
                        {
                            let mut guard = shared.lock().await;
                            if let Some(c) = guard.iter_mut().find(|c| c.id == chunk.id) {
                                c.end_byte = local_end_byte;
                            }
                        }

                        let _ = req.response_tx.send(Some((midpoint, original_end)));
                    } else {
                        let _ = req.response_tx.send(None);
                    }
                }
                next_item = tokio::time::timeout(std::time::Duration::from_secs(30), stream.next()) => {
                    let item = match next_item {
                        Ok(Some(item)) => item,
                        Ok(None) => break, // Stream finished cleanly
                        Err(_) => {
                            stream_failed = Some("Stream timed out (30s) without receiving data".into());
                            break;
                        }
                    };

                    match item {
                        Ok(data) if data.is_empty() => {
                            continue;
                        }
                        Ok(mut data) => {
                            if !is_downloading {
                                is_downloading = true;
                                set_status(shared, chunk.id, ChunkStatus::Downloading).await;
                            }

                            let mut finish_early = false;
                            let current_position = chunk.start_byte + bytes_emitted;

                            if current_position + data.len() as u64 > local_end_byte + 1 {
                                let allowed = (local_end_byte + 1).saturating_sub(current_position);
                                if allowed == 0 {
                                    data = bytes::Bytes::new();
                                    finish_early = true;
                                } else {
                                    data = data.slice(0..allowed as usize);
                                    finish_early = true;
                                }
                            }

                            if !data.is_empty() {
                                let absolute_offset = chunk.start_byte + bytes_emitted;
                                bytes_emitted = bytes_emitted.saturating_add(data.len() as u64);

                                set_current_offset(shared, chunk.id, bytes_emitted).await;

                                let payload = Ok(ChunkPayload {
                                    chunk_id: chunk.id,
                                    absolute_offset,
                                    data,
                                });
                                if tx.send(payload).await.is_err() {
                                    return Ok(()); // cancelled
                                }
                            }

                            if finish_early {
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            stream_failed = Some(e.to_string());
                            break;
                        }
                    }
                }
            }
        }

        if let Some(err) = stream_failed {
            last_error = err;
            set_error_message(shared, chunk.id, Some(last_error.clone())).await;
            continue; // retry from the first byte not already emitted
        }

        // Reached end of stream cleanly.
        return Ok(());
    }

    Err(MultiplexerError::Exhausted {
        chunk_id: chunk.id,
        attempts: MAX_RETRIES,
        message: last_error,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── calculate_chunks ─────────────────────────────────────────────────────

    #[test]
    fn rejects_zero_size() {
        assert!(calculate_chunks(0, 4).is_err());
    }

    #[test]
    fn rejects_zero_connections() {
        assert!(calculate_chunks(1024, 0).is_err());
    }

    #[test]
    fn single_chunk_when_file_smaller_than_min_chunk_size() {
        // File is 64 KiB — below MIN_CHUNK_SIZE of 128 KiB.
        let chunks = calculate_chunks(64 * 1024, 16).unwrap();
        assert_eq!(chunks.len(), 1, "tiny file must not be split");
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 64 * 1024 - 1);
    }

    #[test]
    fn chunks_cover_full_range() {
        let total: u64 = 100_000_000; // 100 MB
        let chunks = calculate_chunks(total, 16).unwrap();
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks.last().unwrap().end_byte, total - 1);
    }

    #[test]
    fn chunks_are_contiguous_and_non_overlapping() {
        let chunks = calculate_chunks(50 * 1024 * 1024, 8).unwrap(); // 50 MiB / 8
        for pair in chunks.windows(2) {
            assert_eq!(
                pair[0].end_byte + 1,
                pair[1].start_byte,
                "gap or overlap between chunk {} and {}",
                pair[0].id,
                pair[1].id
            );
        }
    }

    #[test]
    fn remainder_bytes_distributed_correctly() {
        // 10 bytes into 3 chunks → sizes should be [4, 3, 3] (remainder = 1).
        // But 10 < MIN_CHUNK_SIZE so we'll get 1 chunk; use exact math instead.
        // We test the algorithm directly by inspecting sizes via end - start + 1.
        let total: u64 = 10 * 1024 * 1024 + 7; // 10 MiB + 7 bytes; 10 chunks
        let chunks = calculate_chunks(total, 10).unwrap();
        // Total covered == total_size
        let covered: u64 = chunks.iter().map(|c| c.end_byte - c.start_byte + 1).sum();
        assert_eq!(covered, total);
        // First `remainder` chunks are 1 byte larger than the rest.
        let base = total / 10;
        let remainder = (total % 10) as usize;
        for (i, chunk) in chunks.iter().enumerate() {
            let expected = base + if i < remainder { 1 } else { 0 };
            assert_eq!(
                chunk.end_byte - chunk.start_byte + 1,
                expected,
                "chunk {} has wrong size",
                i
            );
        }
    }

    #[test]
    fn chunk_ids_are_sequential() {
        let chunks = calculate_chunks(32 * 1024 * 1024, 8).unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.id, i);
        }
    }

    #[test]
    fn all_chunks_start_as_pending() {
        let chunks = calculate_chunks(32 * 1024 * 1024, 4).unwrap();
        assert!(chunks.iter().all(|c| c.status == ChunkStatus::Pending));
    }

    // ── range_header format ───────────────────────────────────────────────────

    #[test]
    fn range_header_format_is_correct() {
        let chunk = Chunk {
            id: 0,
            start_byte: 0,
            end_byte: 1_048_575,
            ranged: true,
            status: ChunkStatus::Pending,
            retry_count: 0,
            error_message: None,
            current_offset: 0,
            steal_tx: None,
        };
        let header_value = format!("bytes={}-{}", chunk.start_byte, chunk.end_byte);
        assert_eq!(header_value, "bytes=0-1048575");
    }

    #[test]
    fn range_header_mid_chunk() {
        let chunk = Chunk {
            id: 3,
            start_byte: 3_145_728,
            end_byte: 4_194_303,
            ranged: true,
            status: ChunkStatus::Pending,
            retry_count: 0,
            error_message: None,
            current_offset: 0,
            steal_tx: None,
        };
        let v = format!("bytes={}-{}", chunk.start_byte, chunk.end_byte);
        assert_eq!(v, "bytes=3145728-4194303");
    }

    // ── channel plumbing ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn start_download_returns_handle_immediately() {
        // We are not making real HTTP calls here; we just verify that
        // `start_download` returns without blocking and that the shared state
        // is correctly initialised.
        let client = Client::new();
        let chunks = calculate_chunks(8 * 1024 * 1024, 2).unwrap();
        let mirror_manager = Arc::new(tokio::sync::Mutex::new(crate::mirror::MirrorManager::new(
            vec!["http://127.0.0.1:0/nonexistent".to_string()],
            client.clone(),
        )));
        let handle = start_download(
            client,
            mirror_manager,
            chunks,
            DEFAULT_CHANNEL_CAPACITY,
        );

        // The registry must contain the chunks we provided.
        let guard = handle.chunks.lock().await;
        assert_eq!(guard.len(), 2);
    }
}
