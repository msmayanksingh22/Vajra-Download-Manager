//! Download Task — manages the full lifecycle of a single download.
//!
//! Replaces the old coordinator.rs with:
//! - Pause / Resume support (persists chunk cursors via state file)
//! - Real-time progress events via `tokio::sync::watch`
//! - Single-stream fallback when server doesn't support byte ranges
//! - Post-download checksum verification (optional)
//! - Filename detection from Content-Disposition or URL

#![deny(unsafe_code)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex, RwLock};
use uuid::Uuid;
// ─── Public types ─────────────────────────────────────────────────────────────
pub use vajra_protocol::QueueType;

// ─── Tuning ──────────────────────────────────────────────────────────────────
use crate::constants::*;
use crate::{
    allocator::allocate_file_space,
    multiplexer::{calculate_chunks, start_download, ChunkPayload, DEFAULT_CHANNEL_CAPACITY},
    throttle::{CombinedThrottle, Throttle},
    writer::{start_disk_writer, DataFrame},
};

/// A download request submitted by the frontend or extension.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadRequest {
    /// Full HTTP(S) URL.
    pub url: String,
    /// Fallback mirror URLs
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// Destination directory path. Filename will be auto-detected.
    pub dest_dir: PathBuf,
    /// Override destination filename (if None, detected from URL/headers).
    pub filename: Option<String>,
    /// Optional overall timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Optional connect timeout in seconds
    pub connect_timeout_secs: Option<u64>,
    /// Maximum simultaneous connections for this download.
    pub max_connections: u32,
    /// Optional per-download speed limit in bytes/sec (0 = unlimited).
    pub speed_limit: u64,
    /// Pre-built combined throttle (global + per-download). If None, one is
    /// created automatically from `speed_limit` with an unlimited global bucket.
    #[serde(skip)]
    pub throttle: Option<crate::throttle::CombinedThrottle>,
    /// If true, delete incomplete file on failure.
    pub delete_on_failure: bool,
    #[serde(default)]
    pub use_http3: bool,
    #[serde(default)]
    pub queue_type: QueueType,
    #[serde(default)]
    pub sync_interval_secs: u64,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub cookie_header: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub authorization: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub proxies: Vec<String>,
    #[serde(default)]
    pub local_address: Option<std::net::IpAddr>,
    pub use_ytdlp: bool,
    pub ytdlp_format: Option<String>,
    #[serde(default)]
    pub ytdlp_subtitles: bool,
    #[serde(default)]
    pub ytdlp_playlist: bool,
    /// Expected hash for verification after download completes.
    pub expected_hash: Option<String>,
    /// Auto extract on completion
    pub auto_extract: bool,
    /// Script to run on completion
    pub post_processing_script: Option<String>,
    /// Antivirus scanner executable path
    #[serde(default)]
    pub av_scan_path: Option<String>,
    /// Antivirus scanner arguments
    #[serde(default)]
    pub av_scan_args: Vec<String>,
    /// Schedule task to start at unix timestamp
    #[serde(default)]
    pub schedule_at: Option<i64>,
    /// Daemon configuration for accessing S3 and other settings
    #[serde(default, skip)]
    pub daemon_config: Option<vajra_protocol::DaemonConfig>,
    /// Task priority
    #[serde(default)]
    pub priority: vajra_protocol::Priority,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tcp_multiplexing_opt: bool,
    #[serde(default)]
    pub adaptive_chunk_v2: bool,
}

/// Unique identifier for a download task.
pub type TaskId = Uuid;

/// Live state broadcast to the frontend via Tauri events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub id: TaskId,
    pub url: String,
    pub state: TaskState,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    /// Bytes per second, averaged over the last second.
    pub speed_bps: u64,
    /// Estimated seconds remaining (0 if unknown or complete).
    pub eta_secs: u64,
    /// 0.0 – 1.0
    pub progress_fraction: f64,
    /// Per-chunk completion for the chunk visualizer (index → fraction 0.0–1.0)
    pub chunk_fractions: Vec<f64>,
    pub filename: String,
    pub dest_path: String,
    pub error: Option<String>,
    pub segments: Vec<vajra_protocol::SegmentInfo>,
    pub resume_supported: bool,
    pub hash_result: Option<vajra_protocol::HashResult>,
    pub expected_hash: Option<String>,
    pub queue_type: QueueType,
    pub sync_interval_secs: u64,
    pub tags: Vec<String>,
    pub speed_limit_bps: u64,
}

/// Coarse lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    FetchingMeta,
    SolvingCaptcha,
    Allocating,
    Downloading,
    Pausing,
    Paused,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

/// A running download task handle returned to the queue manager.
#[derive(Clone)]
pub struct DownloadTask {
    pub id: TaskId,
    pub request: DownloadRequest,
    /// Receive the latest progress update.
    pub progress_rx: watch::Receiver<DownloadProgress>,
    /// Send a pause/cancel signal to the active download loop.
    control_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<ControlSignal>>>>,
}

pub enum ControlSignal {
    Pause,
    Cancel,
}

/// Proper error types for download failures.
/// Replaces fragile string-matching on `anyhow::Error` messages.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Download cancelled")]
    Cancelled,
    #[error("Download paused")]
    Paused,
    #[error("Network error: {0}")]
    Network(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Hash mismatch")]
    HashMismatch,
    #[error("Virus detected")]
    VirusDetected,
    #[error("Disk full")]
    DiskFull,
    #[error("Multiplexer error: {0}")]
    Multiplexer(String),
    #[error("Other error: {0}")]
    Other(String),
}

// ─── Task implementation ──────────────────────────────────────────────────────

impl DownloadTask {
    /// Create and immediately start a download in a background Tokio task.
    ///
    /// The returned `DownloadTask` handle lets callers:
    /// - Subscribe to live progress via `progress_rx`
    /// - Pause / cancel via `pause()` / `cancel()`
    pub fn start(request: DownloadRequest) -> Self {
        Self::start_with_id(Uuid::new_v4(), request)
    }

    /// Start a task with a caller-owned stable identifier.
    pub fn start_with_id(id: TaskId, request: DownloadRequest) -> Self {
        let initial_progress = DownloadProgress {
            id,
            url: request.url.clone(),
            state: TaskState::Queued,
            bytes_downloaded: 0,
            total_bytes: 0,
            speed_bps: 0,
            eta_secs: 0,
            progress_fraction: 0.0,
            chunk_fractions: vec![],
            filename: request
                .filename
                .clone()
                .unwrap_or_else(|| detect_filename_from_url(&request.url)),
            dest_path: String::new(),
            error: None,
            segments: vec![],
            resume_supported: false,
            hash_result: None,
            expected_hash: request.expected_hash.clone(),
            queue_type: request.queue_type.clone(),
            sync_interval_secs: request.sync_interval_secs,
            tags: request.tags.clone(),
            speed_limit_bps: request.speed_limit,
        };

        let (progress_tx, progress_rx) = watch::channel(initial_progress);
        let (ctrl_tx, ctrl_rx) = tokio::sync::oneshot::channel::<ControlSignal>();
        let ctrl_tx = Arc::new(Mutex::new(Some(ctrl_tx)));

        // Spawn background work
        let req = request.clone();
        tokio::spawn(async move {
            run_download(id, req, progress_tx, ctrl_rx).await;
        });

        DownloadTask {
            id,
            request,
            progress_rx,
            control_tx: ctrl_tx,
        }
    }

    /// Restore a task in a non-active state (paused, completed, failed, cancelled)
    /// without running a background download loop.
    #[allow(clippy::too_many_arguments)]
    pub fn new_restored(
        id: TaskId,
        request: DownloadRequest,
        state: TaskState,
        bytes_downloaded: u64,
        total_bytes: u64,
        filename: String,
        dest_path: String,
        error: Option<String>,
    ) -> Self {
        let progress = DownloadProgress {
            id,
            url: request.url.clone(),
            state,
            bytes_downloaded,
            total_bytes,
            speed_bps: 0,
            eta_secs: 0,
            progress_fraction: if total_bytes > 0 {
                (bytes_downloaded as f64 / total_bytes as f64).min(1.0)
            } else {
                0.0
            },
            chunk_fractions: vec![],
            filename,
            dest_path,
            error,
            segments: vec![],
            resume_supported: true,
            hash_result: None,
            expected_hash: request.expected_hash.clone(),
            queue_type: request.queue_type.clone(),
            sync_interval_secs: request.sync_interval_secs,
            tags: request.tags.clone(),
            speed_limit_bps: request.speed_limit,
        };
        let (_, progress_rx) = watch::channel(progress);
        Self {
            id,
            request,
            progress_rx,
            control_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Signal the download to pause (state will flush to disk).
    pub async fn pause(&self) {
        let mut lock = self.control_tx.lock().await;
        if let Some(tx) = lock.take() {
            let _ = tx.send(ControlSignal::Pause);
        }
    }

    /// Signal the download to cancel (removes partial file if configured).
    pub async fn cancel(&self) {
        let mut lock = self.control_tx.lock().await;
        if let Some(tx) = lock.take() {
            let _ = tx.send(ControlSignal::Cancel);
        }
    }

    /// Convenience: current progress snapshot.
    pub fn progress(&self) -> DownloadProgress {
        self.progress_rx.borrow().clone()
    }
}

// ─── Core download loop ───────────────────────────────────────────────────────

async fn run_download(
    id: TaskId,
    req: DownloadRequest,
    tx: watch::Sender<DownloadProgress>,
    mut ctrl: tokio::sync::oneshot::Receiver<ControlSignal>,
) {
    let result = if req.url.starts_with("magnet:?") || req.url.ends_with(".torrent") {
        crate::torrent_task::start_torrent(
            id,
            req.url.clone(),
            req.dest_dir.clone(),
            req.filename.clone(),
            tx.clone(),
            ctrl,
        )
        .await
    } else if req.use_ytdlp {
        crate::ytdlp::download_ytdlp(id, &req, &tx, &mut ctrl).await
    } else if req.url.split('?').next().unwrap_or("").ends_with(".m3u8") {
        crate::hls::download_hls(id, &req, &tx, &mut ctrl).await
    } else if req.url.starts_with("ftp://") || req.url.starts_with("ftps://") {
        crate::ftp_task::download_ftp(id, &req, &tx, &mut ctrl).await
    } else {
        download_inner(id, &req, &tx, &mut ctrl).await
    };

    // Publish terminal state
    let mut p = tx.borrow().clone();
    match result {
        Ok(bytes) => {
            // Post Processing: Hash Verification
            if let Some(expected_hash) = &req.expected_hash {
                p.state = TaskState::Verifying;
                let _ = tx.send(p.clone());

                let dest_path = Path::new(&p.dest_path);
                match crate::post_processing::verify_hash(dest_path, expected_hash).await {
                    Ok(matched) => {
                        let algo = if expected_hash.to_lowercase().starts_with("md5:") {
                            "md5"
                        } else {
                            "sha256"
                        };
                        p.hash_result = Some(vajra_protocol::HashResult {
                            matched,
                            algorithm: algo.to_string(),
                            computed: if matched {
                                expected_hash.clone()
                            } else {
                                "Mismatch".to_string()
                            },
                        });
                    }
                    Err(e) => {
                        tracing::error!("Hash verification failed: {}", e);
                    }
                }
            }

            // Auto-detect checksums (SHA256, MD5) if no explicit expected_hash was provided
            if req.expected_hash.is_none() {
                let dest_path = Path::new(&p.dest_path);
                if let Some(res) = crate::cryptography::verify_checksums(dest_path) {
                    p.hash_result = Some(vajra_protocol::HashResult {
                        matched: res.matched,
                        algorithm: res.algorithm,
                        computed: res.computed,
                    });
                }
            }

            // Verify PGP signature if present
            let dest_path = Path::new(&p.dest_path);
            if let Some(verified) = crate::cryptography::verify_pgp_signature(dest_path) {
                tracing::info!(
                    "PGP signature verification outcome for {:?}: {}",
                    dest_path,
                    verified
                );
            }

            let dest_path = PathBuf::from(&p.dest_path);

            // Antivirus Scan
            let mut av_failed = false;
            if let Some(av_path_str) = &req.av_scan_path {
                if !av_path_str.trim().is_empty() {
                    p.state = TaskState::Verifying;
                    let _ = tx.send(p.clone());

                    let av_path = Path::new(av_path_str);
                    tracing::info!("Running antivirus scan with: {:?}", av_path);
                    if let Err(e) = crate::post_processing::run_antivirus_scan(
                        av_path,
                        &req.av_scan_args,
                        &dest_path,
                    )
                    .await
                    {
                        tracing::error!("Antivirus detected a threat or failed: {}", e);
                        p.error = Some(format!("Antivirus alert: {}", e));
                        av_failed = true;
                    }
                }
            }

            if av_failed {
                p.state = TaskState::Failed;
                for seg in &mut p.segments {
                    if seg.status != vajra_protocol::DownloadStatus::Completed {
                        seg.status = vajra_protocol::DownloadStatus::Failed;
                        seg.speed_bps = Some(0);
                    }
                }
            } else {
                // Auto extract
                if req.auto_extract {
                    let ext = dest_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "zip" || ext == "7z" || ext == "rar" {
                        tracing::info!("Auto-extracting archive: {:?}", dest_path);
                        if let Err(e) = crate::post_processing::auto_extract(&dest_path).await {
                            tracing::error!("Auto-extract failed: {}", e);
                        }
                    }
                }

                // Post-processing script
                if let Some(script) = &req.post_processing_script {
                    let script_path = Path::new(script);
                    tracing::info!("Running post-processing script: {:?}", script_path);
                    if let Err(e) =
                        crate::post_processing::run_post_processing_script(script_path, &dest_path)
                            .await
                    {
                        tracing::error!("Post-processing script failed: {}", e);
                    }
                }

                // S3 Upload
                if let Some(cfg) = &req.daemon_config {
                    if cfg.s3_enabled {
                        if let Err(e) = crate::s3::upload_file_to_s3(&dest_path, cfg).await {
                            tracing::error!("S3 upload failed: {}", e);
                            // Set error but keep state Completed or Failed?
                            // For now we just log and set the error message.
                            p.error = Some(format!("S3 upload failed: {}", e));
                        }
                    }
                }

                // Content Pipeline (Phase 5)
                tracing::info!("Running content pipeline for {:?}", dest_path);
                if let Err(e) = crate::content_pipeline::run_pipeline(&dest_path).await {
                    tracing::error!("Content pipeline error: {}", e);
                }

                // Duplicate detection and database hash persistence
                let computed_hash = if let Some(ref hr) = p.hash_result {
                    Some(hr.computed.clone())
                } else {
                    crate::cryptography::compute_sha256(&dest_path).ok()
                };

                if let Some(hash) = computed_hash {
                    if let Ok(db) = crate::db::Database::open(&vajra_protocol::db_path()) {
                        let size = std::fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0);
                        let _ = db.save_file_hash(&p.dest_path, &hash, size);

                        if let Ok(Some(existing_path)) = db.find_duplicate_file(&hash, &p.dest_path)
                        {
                            tracing::warn!(
                                "Content duplicate detected! File has same content hash as: {}",
                                existing_path
                            );
                            p.error = Some(format!("Content duplicate of: {}", existing_path));
                        }
                    }
                }

                p.state = TaskState::Completed;
                p.bytes_downloaded = bytes;
                p.progress_fraction = 1.0;
                p.speed_bps = 0;
                p.eta_secs = 0;
                for seg in &mut p.segments {
                    seg.status = vajra_protocol::DownloadStatus::Completed;
                    seg.bytes_done = seg.allocated_bytes;
                    seg.speed_bps = Some(0);
                }
            }
        }
        Err(e) => {
            // FIX: Use downcast_ref for proper error type classification
            // instead of fragile string matching on error messages.
            let download_err = e.downcast_ref::<DownloadError>();
            let msg = e.to_string();

            match download_err {
                Some(DownloadError::Cancelled) => {
                    p.state = TaskState::Cancelled;
                    for seg in &mut p.segments {
                        if seg.status != vajra_protocol::DownloadStatus::Completed {
                            seg.status = vajra_protocol::DownloadStatus::Failed;
                            seg.speed_bps = Some(0);
                        }
                    }
                }
                Some(DownloadError::Paused) => {
                    p.state = TaskState::Paused;
                    for seg in &mut p.segments {
                        if seg.status != vajra_protocol::DownloadStatus::Completed {
                            seg.status = vajra_protocol::DownloadStatus::Paused;
                            seg.speed_bps = Some(0);
                        }
                    }
                }
                _ => {
                    p.state = TaskState::Failed;
                    p.error = Some(msg.clone());
                    for seg in &mut p.segments {
                        if seg.status != vajra_protocol::DownloadStatus::Completed {
                            seg.status = vajra_protocol::DownloadStatus::Failed;
                            seg.speed_bps = Some(0);
                            seg.error_message = Some(msg.clone());
                        }
                    }
                }
            }
        }
    }
    let _ = tx.send(p);
}

async fn download_inner(
    id: TaskId,
    req: &DownloadRequest,
    tx: &watch::Sender<DownloadProgress>,
    ctrl: &mut tokio::sync::oneshot::Receiver<ControlSignal>,
) -> anyhow::Result<u64> {
    use anyhow::bail;

    // ── Build HTTP client ──────────────────────────────────────────────────
    let mut default_headers = header::HeaderMap::new();
    if let Some(referrer) = &req.referrer {
        if referrer != "strip" {
            default_headers.insert(header::REFERER, header::HeaderValue::from_str(referrer)?);
        }
    }
    if let Some(cookie_header) = &req.cookie_header {
        let mut value = header::HeaderValue::from_str(cookie_header)?;
        value.set_sensitive(true);
        default_headers.insert(header::COOKIE, value);
    }
    if let Some(auth_header) = &req.authorization {
        let mut value = header::HeaderValue::from_str(auth_header)?;
        value.set_sensitive(true);
        default_headers.insert(header::AUTHORIZATION, value);
    }
    let timeout = req
        .timeout_secs
        .map(Duration::from_secs)
        .unwrap_or(REQUEST_TIMEOUT);
    let connect_timeout = req
        .connect_timeout_secs
        .map(Duration::from_secs)
        .unwrap_or(CONNECT_TIMEOUT);

    let mut proxy_list = req.proxies.clone();
    if proxy_list.is_empty() {
        if let Some(p) = &req.proxy {
            proxy_list.push(p.clone());
        }
    }

    // Tor Network Routing Integration (Task 97)
    let is_onion = req.url.contains(".onion") || req.mirrors.iter().any(|m| m.contains(".onion"));
    let mut force_tor = false;
    if let Some(ref config) = req.daemon_config {
        if config.proxy.route_via_tor {
            force_tor = true;
        }
    }

    if (is_onion || force_tor) && proxy_list.is_empty() {
        tracing::info!("Routing download via Tor proxy socks5h://127.0.0.1:9050");
        proxy_list.push("socks5h://127.0.0.1:9050".to_string());
    }

    let mut resolved_doh_ip = None;
    if let Some(ref config) = req.daemon_config {
        if config.dns_over_https {
            if let Ok(url_parsed) = reqwest::Url::parse(&req.url) {
                if let Some(host) = url_parsed.host_str() {
                    tracing::info!("Resolving host '{}' via DNS over HTTPS...", host);
                    match resolve_doh(host).await {
                        Ok(ip) => {
                            tracing::info!("DoH Resolved '{}' to {}", host, ip);
                            resolved_doh_ip = Some((host.to_string(), ip));
                        }
                        Err(e) => {
                            tracing::warn!(
                                "DoH failed for '{}': {}. Falling back to default DNS.",
                                host,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    let mut use_http3 = req.use_http3;
    let create_clients = |u_h3: bool| -> anyhow::Result<reqwest::Client> {
        let create_builder = || -> reqwest::ClientBuilder {
            let keepalive = if req.tcp_multiplexing_opt {
                Some(Duration::from_secs(10))
            } else {
                Some(KEEPALIVE_INTERVAL)
            };
            let user_agents = &[
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"
            ];
            let rotated_ua = if req.user_agent.as_deref() == Some("rotate") {
                let bytes = id.as_bytes();
                let idx = (bytes[0] as usize) % user_agents.len();
                user_agents[idx]
            } else {
                req.user_agent.as_deref().unwrap_or(user_agents[0])
            };

            let mut b = Client::builder()
                .timeout(timeout)
                .connect_timeout(connect_timeout)
                .tcp_keepalive(keepalive)
                .https_only(false)
                .http2_adaptive_window(true)
                .pool_idle_timeout(Some(Duration::from_secs(90)))
                .pool_max_idle_per_host(req.max_connections.max(1) as usize)
                .redirect(reqwest::redirect::Policy::limited(10))
                .default_headers(default_headers.clone())
                .user_agent(rotated_ua);
            if let Some((ref host, ip)) = resolved_doh_ip {
                b = b
                    .resolve(host, std::net::SocketAddr::new(ip, 80))
                    .resolve(host, std::net::SocketAddr::new(ip, 443));
            }
            if u_h3 {
                b = b.http3_prior_knowledge();
            }
            if let Some(addr) = req.local_address {
                b = b.local_address(addr);
            }
            b
        };

        let mut clients = Vec::new();
        if proxy_list.is_empty() {
            clients.push(create_builder().build()?);
        } else {
            for proxy in &proxy_list {
                clients.push(
                    create_builder()
                        .proxy(reqwest::Proxy::all(proxy)?)
                        .build()?,
                );
            }
        }
        Ok(clients[0].clone())
    };

    let mut primary_client = create_clients(use_http3)?;

    let db = crate::db::Database::open(&vajra_protocol::db_path())
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;

    let cached_url = db.load_redirect(&id.to_string()).ok().flatten();
    let mut target_url = cached_url.clone().unwrap_or_else(|| req.url.clone());
    let mut used_cache = cached_url.is_some();

    // ── Probe HEAD ────────────────────────────────────────────────────────
    // We need file metadata: total size, accept-ranges, filename hints.
    // Strategy:
    //   1. Try HEAD — no body, fastest.
    //   2. If HEAD fails (403/405/etc), fall back to GET Range: bytes=0-0.
    //      A 206 response carries Content-Range: bytes 0-0/TOTAL — we parse
    //      TOTAL from that header.  We must NOT use Content-Length here
    //      because 206 responses set Content-Length to the slice size (1),
    //      not the full file size.
    emit(tx, id, |p| p.state = TaskState::FetchingMeta);

    let start_time = std::time::Instant::now();

    let re_sitekey1 = regex::Regex::new(r#"data-sitekey=["']([A-Za-z0-9_-]{40})["']"#).unwrap();
    let re_sitekey2 = regex::Regex::new(r#"sitekey\s*:\s*["']([A-Za-z0-9_-]{40})["']"#).unwrap();

    // Probe result: (response, Option<overridden_total_bytes>)
    let (head, probe_total_bytes_override) = loop {
        let res = match primary_client.head(&target_url).send().await {
            Ok(response) if response.status().is_success() => Some((response, None)),
            _ => {
                // HEAD failed — send a minimal GET to sniff headers without
                // downloading the full body.
                let response = primary_client
                    .get(&target_url)
                    .header(header::RANGE, "bytes=0-0")
                    .send()
                    .await;

                match response {
                    Ok(resp) => {
                        let status = resp.status();
                        let is_html = resp
                            .headers()
                            .get(header::CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.contains("text/html"))
                            .unwrap_or(false);

                        if status == reqwest::StatusCode::FORBIDDEN
                            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                            || (status.is_success() && is_html)
                        {
                            if let Ok(body_text) = resp.text().await {
                                let mut site_key = None;
                                if let Some(cap) = re_sitekey1.captures(&body_text) {
                                    site_key = Some(cap[1].to_string());
                                } else if let Some(cap) = re_sitekey2.captures(&body_text) {
                                    site_key = Some(cap[1].to_string());
                                }

                                if let Some(skey) = site_key {
                                    tracing::info!("Detected reCAPTCHA v2 sitekey: {}", skey);
                                    let captcha_api_key = db
                                        .get_credential_by_domain("2captcha.com")
                                        .ok()
                                        .flatten()
                                        .map(|c| c.password);
                                    if let Some(apikey) = captcha_api_key {
                                        emit(tx, id, |p| p.state = TaskState::SolvingCaptcha);
                                        let solver = crate::captcha::CaptchaSolver::new(apikey);
                                        match solver.solve_recaptcha_v2(&skey, &target_url).await {
                                            Ok(token) => {
                                                tracing::info!("Captcha solved successfully!");
                                                if let Ok(mut url_parsed) =
                                                    url::Url::parse(&target_url)
                                                {
                                                    url_parsed.query_pairs_mut().append_pair(
                                                        "g-recaptcha-response",
                                                        &token,
                                                    );
                                                    target_url = url_parsed.to_string();
                                                }
                                                emit(tx, id, |p| p.state = TaskState::FetchingMeta);
                                                continue;
                                            }
                                            Err(err) => {
                                                tracing::error!("Captcha solving failed: {}", err);
                                            }
                                        }
                                    } else {
                                        tracing::warn!("Captcha detected but no 2captcha.com API key found in vault");
                                    }
                                }
                            }
                            None
                        } else if status.is_success()
                            || status == reqwest::StatusCode::PARTIAL_CONTENT
                        {
                            let real_total: Option<u64> =
                                if status == reqwest::StatusCode::PARTIAL_CONTENT {
                                    resp.headers()
                                        .get(header::CONTENT_RANGE)
                                        .and_then(|v| v.to_str().ok())
                                        .and_then(|v| v.split('/').next_back())
                                        .and_then(|s| s.trim().parse::<u64>().ok())
                                } else {
                                    None
                                };
                            Some((resp, real_total))
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
        };

        if let Some(val) = res {
            break val;
        }

        if use_http3 {
            tracing::warn!(
                "HTTP/3 probe failed for {}. Re-attempting connection fallback with HTTP/2.",
                req.url
            );
            use_http3 = false;
            if let Ok(fallback_client) = create_clients(false) {
                primary_client = fallback_client;
                continue;
            }
        }

        if used_cache {
            tracing::warn!(
                "Cached final URL probe failed. Falling back to original URL: {}",
                req.url
            );
            target_url = req.url.clone();
            used_cache = false;
        } else {
            bail!("Failed to connect or probe remote URL");
        }
    };
    let latency_ms = start_time.elapsed().as_millis() as u64;

    let final_url = head.url().as_str().to_string();
    if final_url != req.url {
        let _ = db.save_redirect(&id.to_string(), &final_url);
    } else {
        let _ = db.delete_redirect(&id.to_string());
    }

    let headers = head.headers();

    let accepts_ranges = head.status() == reqwest::StatusCode::PARTIAL_CONTENT
        || headers
            .get(header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);

    let _etag = headers
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let _last_modified = headers
        .get(header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    // Use the Content-Range override when available (GET bytes=0-0 fallback),
    // otherwise fall back to Content-Length from a HEAD response.
    let total_bytes: u64 = probe_total_bytes_override.unwrap_or_else(|| {
        headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    });

    // Detect filename from Content-Disposition > URL
    let mut filename = req
        .filename
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| detect_filename_from_header(headers))
        .unwrap_or_else(|| detect_filename_from_url(&req.url));

    // Phase 5: Clean up Scene tags / junk characters using basic AI/ML heuristic
    filename = crate::ai::clean_filename_ml(&filename);

    if !filename.contains('.') {
        if let Some(content_type) = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
        {
            let mime = content_type
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let ext = match mime.as_str() {
                "image/jpeg" | "image/jpg" => "jpg",
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                "image/svg+xml" => "svg",
                "image/avif" => "avif",
                "video/mp4" => "mp4",
                "video/webm" => "webm",
                "audio/mpeg" => "mp3",
                "audio/wav" => "wav",
                "application/pdf" => "pdf",
                "application/zip" => "zip",
                "text/html" => "html",
                "application/json" => "json",
                _ => "",
            };
            if !ext.is_empty() {
                filename = format!("{}.{}", filename, ext);
            }
        }
    }

    let dest_path = req.dest_dir.join(&filename);

    let predicted_connections =
        crate::ai::predict_optimal_connections(total_bytes, Some(latency_ms));
    // Use predicted unless user specifically requested a lower/different count (we cap at user's max)
    let max_connections = if req.max_connections < 32 {
        req.max_connections.max(1) as usize
    } else {
        predicted_connections.min(req.max_connections as usize)
    };

    emit(tx, id, |p| {
        p.total_bytes = total_bytes;
        p.filename = filename.clone();
        p.dest_path = dest_path.to_string_lossy().into_owned();
    });

    // ── Allocate disk space ───────────────────────────────────────────────
    if total_bytes > 0 && !dest_path.exists() {
        emit(tx, id, |p| p.state = TaskState::Allocating);
        allocate_file_space(&dest_path, total_bytes).await?;
    } else if !dest_path.exists() {
        // Unknown size — create empty file, will grow as we write
        std::fs::File::create(&dest_path)?;
    }

    // ── Load segment state from SQLite ────────────────────────────────────
    let saved_segments = if accepts_ranges && total_bytes > 0 {
        db.load_segments(&id.to_string()).unwrap_or_default()
    } else {
        let _ = db.delete_segments(&id.to_string());
        Vec::new()
    };

    // ── Calculate initial chunks ──────────────────────────────────────────
    emit(tx, id, |p| p.state = TaskState::Downloading);

    let chunks = if !saved_segments.is_empty() {
        // Resume from explicitly saved chunks
        saved_segments
            .iter()
            .map(|c| crate::multiplexer::Chunk {
                id: c.chunk_id,
                start_byte: c.start_byte.unwrap_or(0),
                end_byte: c.end_byte.unwrap_or(total_bytes.saturating_sub(1)),
                ranged: accepts_ranges,
                status: crate::multiplexer::ChunkStatus::Pending,
                retry_count: 0,
                error_message: None,
                current_offset: c.bytes_written,
                steal_tx: None,
            })
            .collect::<Vec<_>>()
    } else if accepts_ranges && total_bytes > 0 {
        calculate_chunks(total_bytes, max_connections)?
    } else {
        // Single-stream fallback
        vec![crate::multiplexer::Chunk {
            id: 0,
            start_byte: 0,
            end_byte: if total_bytes > 0 {
                total_bytes.saturating_sub(1)
            } else {
                u64::MAX
            },
            ranged: false, // Must not send Range header if no ranges are accepted or size is unknown
            status: crate::multiplexer::ChunkStatus::Pending,
            retry_count: 0,
            error_message: None,
            current_offset: 0,
            steal_tx: None,
        }]
    };

    let num_chunks = chunks.len();

    let resumed_bytes: Vec<u64> = chunks
        .iter()
        .map(|chunk| {
            let chunk_size = chunk.end_byte - chunk.start_byte + 1;
            saved_segments
                .iter()
                .find(|p| p.chunk_id == chunk.id)
                .map(|progress| progress.bytes_written.min(chunk_size))
                .unwrap_or(0)
        })
        .collect();

    let already_downloaded: u64 = resumed_bytes.iter().sum();

    let pending_chunks: Vec<_> = chunks
        .iter()
        .filter_map(|chunk| {
            let resume_offset = resumed_bytes.get(chunk.id).copied().unwrap_or(0);
            let next_start = chunk.start_byte + resume_offset;
            (next_start <= chunk.end_byte).then_some(crate::multiplexer::Chunk {
                id: chunk.id,
                start_byte: next_start,
                end_byte: chunk.end_byte,
                ranged: chunk.ranged,
                status: crate::multiplexer::ChunkStatus::Pending,
                retry_count: 0,
                error_message: None,
                current_offset: 0,
                steal_tx: None,
            })
        })
        .collect();

    let initial_segments: Vec<vajra_protocol::SegmentInfo> = chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let chunk_size = if total_bytes > 0 {
                chunk.end_byte - chunk.start_byte + 1
            } else {
                0
            };
            let bytes_done = resumed_bytes[index];
            let status = if bytes_done >= chunk_size && total_bytes > 0 {
                vajra_protocol::DownloadStatus::Completed
            } else {
                vajra_protocol::DownloadStatus::Idle
            };
            vajra_protocol::SegmentInfo {
                id: chunk.id,
                start: chunk.start_byte,
                end: chunk.end_byte,
                bytes_done,
                allocated_bytes: chunk_size,
                status,
                thread_index: index + 1,
                speed_bps: Some(0),
                retry_count: 0,
                error_message: None,
            }
        })
        .collect();

    emit(tx, id, |p| {
        p.segments = initial_segments;
        p.resume_supported = accepts_ranges;
    });

    // Per-chunk byte counters for progress (stored behind RwLock so bridge + speed sampler share)
    let chunk_bytes: Arc<RwLock<Vec<u64>>> = Arc::new(RwLock::new(vec![0; num_chunks]));

    let (writer_tx, writer_rx) = tokio::sync::mpsc::channel::<DataFrame>(WRITER_CHANNEL_CAPACITY);

    // ── Start multiplexed download ────────────────────────────────────────
    let mut mirror_urls = vec![final_url.clone()];
    mirror_urls.extend(req.mirrors.clone());
    let mirror_manager = Arc::new(tokio::sync::Mutex::new(crate::mirror::MirrorManager::new(
        mirror_urls,
        primary_client.clone(),
    )));

    let mux_handle = start_download(
        primary_client,
        mirror_manager,
        pending_chunks,
        DEFAULT_CHANNEL_CAPACITY,
    );
    let mut mux_rx = mux_handle.receiver;

    // ── Bridge task (mux → writer) with RAM-buffered I/O (Phase 3C) ──────
    let chunk_bytes_bridge = Arc::clone(&chunk_bytes);
    let throttle = req
        .throttle
        .clone()
        .unwrap_or_else(|| CombinedThrottle::new(Throttle::unlimited(), req.speed_limit));
    let bridge: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        // per-chunk buffer: chunk_id → (buffer_start_offset, data)
        let mut buf_map: HashMap<usize, (u64, Vec<u8>)> = HashMap::new();
        let mut flush_tick = tokio::time::interval(Duration::from_millis(FLUSH_TICK_MS));
        flush_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        /// Drain one chunk's buffer into the writer channel.
        async fn flush_one(
            chunk_id: usize,
            buf_map: &mut HashMap<usize, (u64, Vec<u8>)>,
            writer_tx: &tokio::sync::mpsc::Sender<DataFrame>,
        ) -> anyhow::Result<()> {
            if let Some((offset, data)) = buf_map.remove(&chunk_id) {
                if !data.is_empty() {
                    let frame = DataFrame {
                        absolute_offset: offset,
                        payload: bytes::Bytes::from(data),
                    };
                    if writer_tx.send(frame).await.is_err() {
                        anyhow::bail!("Writer channel closed");
                    }
                }
            }
            Ok(())
        }

        loop {
            tokio::select! {
                biased;

                maybe = mux_rx.recv() => {
                    let result = match maybe {
                        Some(r) => r,
                        None => {
                            // Channel closed — flush all remaining buffers
                            let ids: Vec<usize> = buf_map.keys().copied().collect();
                            for id in ids {
                                flush_one(id, &mut buf_map, &writer_tx).await?;
                            }
                            break;
                        }
                    };

                    let payload: ChunkPayload =
                        result.map_err(|e| anyhow::anyhow!("Chunk download failed: {e}"))?;

                    let len = payload.data.len() as u64;

                    // Token-bucket throttle
                    throttle.acquire(len).await;

                    // Accumulate into RAM buffer
                    let entry = buf_map.entry(payload.chunk_id).or_insert_with(|| (payload.absolute_offset, Vec::new()));
                    entry.1.extend_from_slice(&payload.data);
                    let new_len = entry.1.len();

                    // Update per-chunk byte counters for progress
                    {
                        let mut guard = chunk_bytes_bridge.write().await;
                        if payload.chunk_id >= guard.len() {
                            guard.resize(payload.chunk_id + 1, 0);
                        }
                        guard[payload.chunk_id] += len;
                    }

                    // Flush if buffer hit threshold
                    if new_len >= RAM_FLUSH_THRESHOLD_BYTES {
                        let id = payload.chunk_id;
                        flush_one(id, &mut buf_map, &writer_tx).await?;
                    }
                }

                // Periodic flush every 250ms
                _ = flush_tick.tick() => {
                    let ids: Vec<usize> = buf_map.keys().copied().collect();
                    for id in ids {
                        flush_one(id, &mut buf_map, &writer_tx).await?;
                    }
                }
            }
        }
        Ok(())
    });

    // ── Writer task ───────────────────────────────────────────────────────
    let dest_for_writer = dest_path.clone();
    let writer_fut = tokio::spawn(async move {
        start_disk_writer(&dest_for_writer, writer_rx)
            .await
            .map_err(|e| anyhow::anyhow!("Write failed: {e}"))
    });

    // ── Progress + control polling loop ───────────────────────────────────
    let mut speed_window = SpeedWindow::new();
    let mut segment_speed_windows: Vec<SpeedWindow> =
        (0..num_chunks).map(|_| SpeedWindow::new()).collect();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Compute totals
                let guard = chunk_bytes.read().await;
                let total_done: u64 = already_downloaded + guard.iter().sum::<u64>();

                let speed = speed_window.update(total_done);
                let eta = if speed > 0 && total_bytes > total_done {
                    (total_bytes - total_done) / speed
                } else {
                    0
                };
                let fraction = if total_bytes > 0 {
                    total_done as f64 / total_bytes as f64
                } else {
                    0.0
                };

                let active_chunks = mux_handle.chunks.lock().await;
                let chunk_fractions: Vec<f64> = active_chunks.iter().map(|chunk| {
                    let remaining_size = chunk.end_byte.saturating_sub(chunk.start_byte) + 1;
                    let session_bytes = guard.get(chunk.id).copied().unwrap_or(0);
                    let initial_bytes = if chunk.id < resumed_bytes.len() { resumed_bytes[chunk.id] } else { 0 };
                    let original_size = remaining_size + initial_bytes;
                    let bytes_done = initial_bytes + session_bytes;

                    if original_size > 0 {
                        (bytes_done as f64 / original_size as f64).min(1.0)
                    } else {
                        0.0
                    }
                }).collect();

                let segment_speeds: Vec<u64> = active_chunks.iter().map(|chunk| {
                    if chunk.id >= segment_speed_windows.len() {
                        segment_speed_windows.resize_with(chunk.id + 1, SpeedWindow::new);
                    }
                    let session_bytes = guard.get(chunk.id).copied().unwrap_or(0);
                    segment_speed_windows[chunk.id].update(session_bytes)
                }).collect();
                drop(guard);

                let segments: Vec<vajra_protocol::SegmentInfo> = active_chunks.iter().enumerate().map(|(index, chunk)| {
                    let remaining_size = chunk.end_byte.saturating_sub(chunk.start_byte) + 1;
                    let initial_bytes = if chunk.id < resumed_bytes.len() { resumed_bytes[chunk.id] } else { 0 };
                    let original_size = remaining_size + initial_bytes;
                    let bytes_done = (chunk_fractions[index] * original_size as f64) as u64;

                    let (status, retry_count, error_message) = if bytes_done >= original_size && total_bytes > 0 {
                        (vajra_protocol::DownloadStatus::Completed, 0, None)
                    } else {
                        let status = match &chunk.status {
                            crate::multiplexer::ChunkStatus::Pending => vajra_protocol::DownloadStatus::Idle,
                            crate::multiplexer::ChunkStatus::Connecting => vajra_protocol::DownloadStatus::Connecting,
                            crate::multiplexer::ChunkStatus::Downloading => vajra_protocol::DownloadStatus::Downloading,
                            crate::multiplexer::ChunkStatus::Completed => vajra_protocol::DownloadStatus::Completed,
                            crate::multiplexer::ChunkStatus::Failed { .. } => vajra_protocol::DownloadStatus::Failed,
                        };
                        (status, chunk.retry_count, chunk.error_message.clone())
                    };

                    vajra_protocol::SegmentInfo {
                        id: chunk.id,
                        start: chunk.start_byte.saturating_sub(initial_bytes),
                        end: chunk.end_byte,
                        bytes_done,
                        allocated_bytes: original_size,
                        speed_bps: Some(segment_speeds[index]),
                        status,
                        thread_index: 0,
                        retry_count,
                        error_message,
                    }
                }).collect();
                drop(active_chunks);

                emit(tx, id, |p| {
                    p.bytes_downloaded = total_done;
                    p.speed_bps = speed;
                    p.eta_secs = eta;
                    p.progress_fraction = fraction.min(1.0);
                    p.chunk_fractions = chunk_fractions;
                    p.segments = segments;
                });

                // Check if both tasks finished
                if bridge.is_finished() && writer_fut.is_finished() {
                    break;
                }
            }

            signal = &mut *ctrl => {
                match signal {
                    Ok(ControlSignal::Pause) => {
                        // Save state and abort
                        let guard = chunk_bytes.read().await;
                        let active_chunks = mux_handle.chunks.lock().await;
                        for chunk in active_chunks.iter() {
                            let session = guard.get(chunk.id).copied().unwrap_or(0);
                            let initial = if chunk.id < resumed_bytes.len() {
                                resumed_bytes[chunk.id]
                            } else {
                                0
                            };
                            let written = initial + session;
                            let _ = db.save_segment(
                                &id.to_string(),
                                chunk.id,
                                chunk.start_byte,
                                chunk.end_byte,
                                written,
                            );
                        }
                        drop(active_chunks);
                        drop(guard);
                        bridge.abort();
                        writer_fut.abort();
                        return Err(DownloadError::Paused.into());
                    }
                    Ok(ControlSignal::Cancel) => {
                        bridge.abort();
                        writer_fut.abort();
                        // Remove partial file
                        if req.delete_on_failure {
                            let _ = std::fs::remove_file(&dest_path);
                        }
                        let _ = db.delete_segments(&id.to_string());
                        return Err(DownloadError::Cancelled.into());
                    }
                    Err(_) => {} // sender dropped — continue normally
                }
            }
        }
    }

    // Propagate any errors from bridge or writer
    bridge.await??;
    writer_fut.await??;

    // Clean up segment tracking on success
    let _ = db.delete_segments(&id.to_string());

    let guard = chunk_bytes.read().await;
    let total_done = already_downloaded + guard.iter().sum::<u64>();
    drop(guard);

    // Post-processing is handled in run_download() to cover all protocol types

    Ok(total_done)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn emit<F: FnOnce(&mut DownloadProgress)>(tx: &watch::Sender<DownloadProgress>, _id: TaskId, f: F) {
    tx.send_modify(f);
}

fn detect_filename_from_url(url: &str) -> String {
    let base = url.split('#').next().unwrap_or(url);
    let base = base.split('?').next().unwrap_or(base);
    let name = base
        .split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    percent_decode(name).unwrap_or_else(|| name.to_string())
}

fn detect_filename_from_header(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let cd = headers.get(header::CONTENT_DISPOSITION)?.to_str().ok()?;

    let mut plain_filename: Option<String> = None;

    for part in cd.split(';') {
        let part = part.trim();
        // RFC 5987 extended value — highest priority
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            if let Some(rest) = rest
                .strip_prefix("UTF-8''")
                .or_else(|| rest.strip_prefix("utf-8''"))
            {
                if let Some(decoded) = percent_decode(rest) {
                    return Some(decoded);
                }
            }
        }
        // Plain filename (may be RFC 2047 encoded-word)
        if let Some(rest) = part.strip_prefix("filename=") {
            let raw = rest.trim_matches('"').to_string();
            // Decode RFC 2047 encoded-word: =?charset?encoding?text?=
            plain_filename = Some(decode_rfc2047(&raw));
        }
    }
    plain_filename
}

/// Decode RFC 2047 encoded-words. Handles Q-encoding and B-encoding.
/// e.g. =?UTF-8?Q?avast=5Fone=5Ffree.exe?= → avast_one_free.exe
fn decode_rfc2047(input: &str) -> String {
    let mut result = input.to_string();
    // Find all =?...?...?...?= blocks
    while let (Some(start), Some(end)) = (result.find("=?"), result.find("?=")) {
        if end <= start {
            break;
        }
        let word = &result[start..end + 2];
        let inner = &word[2..word.len() - 2]; // strip =? and ?=
        let mut parts = inner.splitn(3, '?');
        let _charset = parts.next().unwrap_or("");
        let encoding = parts.next().unwrap_or("");
        let text = parts.next().unwrap_or("");
        let decoded = match encoding.to_uppercase().as_str() {
            "Q" => decode_q_encoding(text),
            "B" => {
                // base64 decode
                let cleaned = text.replace(' ', "+");
                base64_decode(&cleaned).unwrap_or_else(|| text.to_string())
            }
            _ => text.to_string(),
        };
        result = format!("{}{}{}", &result[..start], decoded, &result[end + 2..]);
    }
    result
}

/// Quoted-Printable decoder for RFC 2047 Q encoding.
/// Underscores are decoded as spaces; =XX as hex bytes.
fn decode_q_encoding(input: &str) -> String {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'_' => {
                out.push(b' ');
                i += 1;
            }
            b'=' if i + 2 < bytes.len() => {
                if let Ok(byte) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
                {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decode a base64-encoded string using the standard alphabet.
///
/// Replaces the previous hand-rolled implementation (BUG-14) which silently
/// truncated inputs whose length was not a multiple of four.
fn base64_decode(input: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    // Replace RFC 2047 space-in-encoded-word with '+' before decoding.
    let normalised = input.replace(' ', "+");
    let bytes = STANDARD.decode(normalised.as_bytes()).ok()?;
    String::from_utf8(bytes).ok()
}

/// Percent-decode a URL-encoded string, correctly handling multi-byte UTF-8.
///
/// Previous implementation (BUG-15) cast each decoded byte directly to `char`,
/// which corrupted any multi-byte sequence (e.g., CJK characters in filenames).
/// This version collects raw bytes and decodes them as UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// Rolling speed window — calculates bytes/sec over the last ~1 second.
struct SpeedWindow {
    last_bytes: u64,
    last_time: Instant,
    smoothed: u64,
    history: std::collections::VecDeque<u64>,
}

impl SpeedWindow {
    fn new() -> Self {
        Self {
            last_bytes: 0,
            last_time: Instant::now(),
            smoothed: 0,
            history: std::collections::VecDeque::with_capacity(20),
        }
    }

    fn update(&mut self, total_bytes: u64) -> u64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        if elapsed < 0.01 {
            return self.smoothed;
        }
        let delta = total_bytes.saturating_sub(self.last_bytes);
        let instant_speed = (delta as f64 / elapsed) as u64;

        self.smoothed = ((0.8 * self.smoothed as f64) + (0.2 * instant_speed as f64)) as u64;
        self.last_bytes = total_bytes;
        self.last_time = now;

        if self.history.len() >= 20 {
            self.history.pop_front();
        }
        self.history.push_back(self.smoothed);

        let non_zero_samples: Vec<u64> = self.history.iter().copied().filter(|&s| s > 0).collect();
        if !non_zero_samples.is_empty() {
            non_zero_samples.iter().sum::<u64>() / non_zero_samples.len() as u64
        } else {
            self.smoothed
        }
    }
}

async fn resolve_doh(host: &str) -> anyhow::Result<std::net::IpAddr> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let url = format!("https://cloudflare-dns.com/dns-query?name={}&type=A", host);
    let res = client
        .get(&url)
        .header("Accept", "application/dns-json")
        .send()
        .await?;

    #[derive(serde::Deserialize)]
    struct DohResponse {
        #[serde(rename = "Answer")]
        answer: Option<Vec<DohAnswer>>,
    }

    #[derive(serde::Deserialize)]
    struct DohAnswer {
        data: String,
    }

    let json: DohResponse = res.json().await?;
    if let Some(answers) = json.answer {
        if let Some(ans) = answers.first() {
            let ip: std::net::IpAddr = ans.data.parse()?;
            return Ok(ip);
        }
    }
    anyhow::bail!("No DNS answer found")
}
