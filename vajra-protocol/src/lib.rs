//! Vajra Protocol — Shared REST API schema types and platform path utilities.
//!
//! Used by `vajra-daemon` (server) and `vajra-cli`/`vajra-ui` (clients).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const API_VERSION: u32 = 1;
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Default port the daemon listens on. Override with VAJRA_PORT env var or config.
pub const DEFAULT_PORT: u16 = 6277;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, utoipa::ToSchema)]
pub enum QueueType {
    #[default]
    Standard,
    Synchronization,
}

// ─── REST Request types ───────────────────────────────────────────────────────

/// POST /api/v1/downloads
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AddDownloadRequest {
    pub url: String,
    /// Destination directory. None = use configured default.
    pub output_dir: Option<String>,
    /// Override the filename. None = infer from URL/Content-Disposition.
    pub filename: Option<String>,
    /// Extra headers to forward (Cookie, Referer, etc.) captured by the browser extension.
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// SHA-256 / MD5 / SHA-1 expected hash (format: "sha256:hexvalue")
    pub expected_hash: Option<String>,
    /// Number of parallel segments (1–32). Default = 8.
    pub max_connections: Option<u32>,
    /// Per-download speed cap in bytes/sec. None = unlimited.
    pub speed_limit_bps: Option<u64>,
    /// Priority ordering in the queue.
    #[serde(default)]
    pub priority: Priority,
    /// Unix timestamp to start this download. None = start immediately.
    pub schedule_at: Option<i64>,
    #[serde(default)]
    pub use_http3: bool,
    #[serde(default)]
    pub use_ytdlp: bool,
    pub ytdlp_format: Option<String>,
    #[serde(default)]
    pub ytdlp_subtitles: bool,
    #[serde(default)]
    pub ytdlp_playlist: bool,
    /// Whether to automatically extract archive files (zip, etc) when done
    #[serde(default)]
    pub auto_extract: bool,
    /// Optional script to run after completion
    pub post_processing_script: Option<String>,
    #[serde(default)]
    pub queue_type: Option<QueueType>,
    pub sync_interval_secs: Option<u64>,
    pub tags: Option<Vec<String>>,
}

/// PATCH /api/v1/downloads/:id
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PatchDownloadRequest {
    pub action: Option<DownloadAction>,
    pub speed_limit_bps: Option<Option<u64>>,
    pub max_connections: Option<u32>,
    pub priority: Option<Priority>,
    pub url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub queue_type: Option<String>,
    pub sync_interval_secs: Option<u64>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DownloadAction {
    Pause,
    Resume,
    Cancel,
    Retry,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
}

/// POST /api/v1/inspect
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InspectRequest {
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

// ─── REST Response types ──────────────────────────────────────────────────────

/// 201 response to POST /api/v1/downloads
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AddDownloadResponse {
    pub id: Uuid,
    pub status: String,
    pub url: String,
    pub filename: Option<String>,
    pub created_at: i64,
}

/// Single download item in GET /api/v1/downloads and GET /api/v1/downloads/:id
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DownloadInfo {
    pub id: Uuid,
    pub status: DownloadStatus,
    pub url: String,
    pub output_path: Option<String>,
    pub filename: String,
    pub total_bytes: Option<u64>,
    pub bytes_done: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub progress_pct: f64,
    pub connections_active: u8,
    pub segments: Vec<SegmentInfo>,
    pub hash_result: Option<HashResult>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
    /// Last N speed samples (500ms intervals) for the sparkline.
    pub speed_history: Vec<u64>,
    /// Queue priority for this download.
    #[serde(default)]
    pub priority: Priority,
    /// The type of queue this download is in.
    #[serde(default)]
    pub queue_type: String,
    #[serde(default)]
    pub sync_interval_secs: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub speed_limit_bps: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Downloading,
    Connecting,
    Completed,
    Paused,
    Failed,
    Idle,
    Verifying,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DownloadStatus::Downloading => "downloading",
            DownloadStatus::Connecting => "connecting",
            DownloadStatus::Completed => "completed",
            DownloadStatus::Paused => "paused",
            DownloadStatus::Failed => "failed",
            DownloadStatus::Idle => "idle",
            DownloadStatus::Verifying => "verifying",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SegmentInfo {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub bytes_done: u64,
    pub allocated_bytes: u64,
    pub status: DownloadStatus,
    pub thread_index: usize,
    pub speed_bps: Option<u64>,
    pub retry_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HashResult {
    pub matched: bool,
    pub algorithm: String,
    pub computed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DownloadProgressResponse {
    pub event: String,
    pub download_id: Uuid,
    pub url: String,
    pub filename: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub status: DownloadStatus,
    pub resume_supported: bool,
    pub segments: Vec<SegmentInfo>,
    pub error: Option<String>,
    pub speed_limit_bps: u64,
}

/// GET /api/v1/downloads (paginated list)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DownloadList {
    pub items: Vec<DownloadInfo>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

/// GET /api/v1/stats
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StatsResponse {
    pub active_count: usize,
    pub queued_count: usize,
    pub paused_count: usize,
    pub complete_today: usize,
    pub failed_today: usize,
    pub aggregate_speed_bps: u64,
    pub aggregate_limit_bps: Option<u64>,
    pub total_downloaded_bytes: u64,
    pub daemon_uptime_seconds: u64,
    #[serde(default)]
    pub speed_history: Vec<u64>,
}

/// 200 response to POST /api/v1/inspect
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InspectResponse {
    pub effective_url: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub total_bytes: Option<u64>,
    pub accepts_ranges: bool,
    pub ytdlp_supported: bool,
}

// ─── Import types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportEf2Request {
    pub content: String,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportEf2Response {
    pub imported_count: usize,
    pub errors: Vec<String>,
}

// ─── Vault types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AddVaultCredentialRequest {
    pub domain: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VaultCredentialResponse {
    pub id: String,
    pub domain: String,
    pub username: String,
    pub created_at: i64,
}

// ─── RSS types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AddRssFeedRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RssFeed {
    pub id: String,
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

// ─── SSE Event types ──────────────────────────────────────────────────────────

/// Emitted on GET /api/v1/events and GET /api/v1/downloads/:id/events
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DaemonEvent {
    Progress {
        download_id: Uuid,
        url: String,
        filename: String,
        total_bytes: Option<u64>,
        downloaded_bytes: u64,
        speed_bps: u64,
        eta_seconds: Option<u64>,
        status: DownloadStatus,
        resume_supported: bool,
        segments: Vec<SegmentInfo>,
        error: Option<String>,
    },
    StateChange {
        id: Uuid,
        status: DownloadStatus,
        output_path: Option<String>,
        error: Option<String>,
    },
    HashResult {
        id: Uuid,
        matched: bool,
        algorithm: String,
        computed: String,
    },
    Added {
        id: Uuid,
        url: String,
        filename: String,
    },
    Removed {
        id: Uuid,
    },
    Intercepted {
        url: String,
        filename: String,
    },
    BatchProgress {
        downloads: Vec<BatchProgressItem>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BatchProgressItem {
    pub download_id: Uuid,
    pub url: String,
    pub filename: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub status: DownloadStatus,
    pub resume_supported: bool,
    pub segments: Vec<SegmentInfo>,
    pub error: Option<String>,
    pub speed_limit_bps: u64,
}

// ─── Config schema ────────────────────────────────────────────────────────────

/// Maps file extensions to a target directory for automatic categorization.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CategoryRule {
    /// Extensions without dot, e.g. ["mp4", "mkv", "avi"]
    pub extensions: Vec<String>,
    /// Absolute path to save directory, e.g. "C:\\Users\\me\\Videos"
    pub output_dir: String,
    /// Human-readable label, e.g. "Videos"
    pub label: String,
}

/// HTTP/SOCKS proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct ProxyConfig {
    /// Legacy single proxy URL
    pub url: Option<String>,
    /// List of proxy URLs for rotation
    #[serde(default)]
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Use system proxy from Windows Internet Options
    #[serde(default)]
    pub use_system_proxy: bool,
    /// Route all downloads over the local Tor SOCKS5 network
    #[serde(default)]
    pub route_via_tor: bool,
}

/// Action to perform when download queue finishes.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostQueueAction {
    #[default]
    None,
    ExitApp,
    Sleep,
    Hibernate,
    Shutdown,
}

/// Duplicate file handling strategy.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateAction {
    #[default]
    AutoRename,
    Overwrite,
    Prompt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct S3Config {
    pub enabled: bool,
    pub bucket: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DaemonConfig {
    #[serde(default = "default_output_dir")]
    pub default_output_dir: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: u8,
    pub global_speed_limit_bps: Option<u64>,
    #[serde(default = "default_connections")]
    pub default_max_connections: u8,
    #[serde(default = "default_port")]
    pub listen_port: u16,
    pub api_token: Option<String>,
    #[serde(default = "bool_true")]
    pub auto_start_on_login: bool,
    #[serde(default = "bool_true")]
    pub notifications_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    #[serde(default = "bool_true")]
    pub sound_on_complete: bool,
    #[serde(default)]
    pub default_use_http3: bool,
    pub captcha_api_key: Option<String>,
    #[serde(default)]
    pub dns_over_https: bool,

    // ── New fields ────────────────────────────────────────────────────────────
    /// Auto-categorization rules (extension → folder mapping)
    #[serde(default)]
    pub category_rules: Vec<CategoryRule>,

    /// Network interface to bind downloads to (e.g. "eth0")
    pub bind_interface: Option<String>,

    /// Interface name for the VPN kill switch. If specified and not found, downloads are paused.
    pub vpn_interface: Option<String>,

    /// Global S3 settings for uploads
    #[serde(default)]
    pub s3: S3Config,

    /// Proxy configuration
    #[serde(default)]
    pub proxy: ProxyConfig,

    /// Action to take when queue empties
    #[serde(default)]
    pub post_queue_action: PostQueueAction,

    /// Is the scheduler enabled?
    #[serde(default)]
    pub scheduler_enabled: bool,

    /// Scheduler start time in HH:MM
    pub scheduler_start_time: Option<String>,

    /// Scheduler stop time in HH:MM
    pub scheduler_stop_time: Option<String>,

    /// How to handle duplicate filenames
    #[serde(default)]
    pub duplicate_action: DuplicateAction,

    /// Path to antivirus executable for post-download scan
    /// e.g. "C:\\Program Files\\Windows Defender\\MpCmdRun.exe"
    pub av_scan_path: Option<String>,

    /// Extra args for AV scan, e.g. ["-Scan", "-ScanType", "3", "-File"]
    #[serde(default)]
    pub av_scan_args: Vec<String>,

    /// Temp directory for chunk assembly (uses default_output_dir if None)
    pub temp_dir: Option<String>,

    /// Fair Access Policy enabled flag
    #[serde(default)]
    pub fap_enabled: bool,

    /// Fair Access Policy quota limit in MB
    #[serde(default = "default_fap_quota")]
    pub fap_quota_mb: u64,

    /// Fair Access Policy window in hours
    #[serde(default = "default_fap_window")]
    pub fap_window_hours: u64,

    /// Site-specific connection limits: domain → max_connections
    #[serde(default)]
    pub site_connection_limits: std::collections::HashMap<String, u8>,

    /// Extension blacklist domains (never intercept these)
    #[serde(default)]
    pub blacklist_domains: Vec<String>,

    /// Enable global clipboard monitor for URLs
    #[serde(default = "bool_true")]
    pub enable_clipboard_monitor: bool,

    /// Automatically extract .zip / .rar / .7z downloads upon completion
    #[serde(default)]
    pub auto_extract: bool,

    /// Custom OS script to execute when a download finishes
    pub post_process_script: Option<String>,

    /// Webhook URLs to hit on Download Completed / Failed events
    #[serde(default)]
    pub webhooks: Vec<String>,

    /// Secret key used to sign webhook payloads (HMAC-SHA256)
    pub webhook_secret: Option<String>,

    // S3 / Remote Storage
    pub s3_enabled: bool,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    #[serde(default)]
    pub s3_delete_local: bool,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            default_output_dir: default_output_dir(),
            max_concurrent_downloads: default_max_concurrent(),
            global_speed_limit_bps: None,
            default_max_connections: default_connections(),
            listen_port: DEFAULT_PORT,
            api_token: None,
            auto_start_on_login: true,
            notifications_enabled: true,
            tls_cert_path: None,
            tls_key_path: None,
            sound_on_complete: true,
            default_use_http3: false,
            captcha_api_key: None,
            dns_over_https: false,
            category_rules: default_category_rules(),
            bind_interface: None,
            vpn_interface: None,
            s3: S3Config::default(),
            proxy: ProxyConfig::default(),
            post_queue_action: PostQueueAction::None,
            scheduler_enabled: false,
            scheduler_start_time: None,
            scheduler_stop_time: None,
            duplicate_action: DuplicateAction::AutoRename,
            av_scan_path: None,
            av_scan_args: vec![
                "-Scan".into(),
                "-ScanType".into(),
                "3".into(),
                "-File".into(),
            ],
            temp_dir: None,
            fap_enabled: false,
            fap_quota_mb: default_fap_quota(),
            fap_window_hours: default_fap_window(),
            site_connection_limits: std::collections::HashMap::new(),
            blacklist_domains: vec![],
            enable_clipboard_monitor: true,
            auto_extract: false,
            post_process_script: None,
            webhooks: vec![],
            webhook_secret: None,
            s3_enabled: false,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            s3_access_key: None,
            s3_secret_key: None,
            s3_delete_local: false,
            max_retries: default_max_retries(),
        }
    }
}

pub fn default_output_dir() -> String {
    dirs_next::download_dir()
        .or_else(dirs_next::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .into_owned()
}
fn default_max_concurrent() -> u8 {
    3
}
fn default_connections() -> u8 {
    8
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn bool_true() -> bool {
    true
}
fn default_fap_quota() -> u64 {
    150
}
fn default_fap_window() -> u64 {
    4
}
fn default_max_retries() -> u32 {
    2
}

/// Default auto-categorization rules matching common file types to OS standard folders.
fn default_category_rules() -> Vec<CategoryRule> {
    let dl = dirs_next::download_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .into_owned();

    vec![
        CategoryRule {
            label: "Videos".into(),
            extensions: vec![
                "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "m2ts",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            output_dir: dirs_next::video_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| dl.clone()),
        },
        CategoryRule {
            label: "Music".into(),
            extensions: vec!["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "opus"]
                .into_iter()
                .map(String::from)
                .collect(),
            output_dir: dirs_next::audio_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| dl.clone()),
        },
        CategoryRule {
            label: "Documents".into(),
            extensions: vec![
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "epub",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            output_dir: dirs_next::document_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| dl.clone()),
        },
        CategoryRule {
            label: "Software".into(),
            extensions: vec![
                "exe", "msi", "msix", "dmg", "pkg", "deb", "rpm", "apk", "appimage",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            output_dir: dl.clone(),
        },
        CategoryRule {
            label: "Archives".into(),
            extensions: vec![
                "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst", "iso", "img",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            output_dir: dl,
        },
    ]
}

// ─── Platform path helpers ────────────────────────────────────────────────────

pub fn app_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VAJRA_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs_next::data_local_dir()
        .or_else(dirs_next::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Vajra")
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

pub fn db_path() -> PathBuf {
    app_data_dir().join("vajra.db")
}

pub fn token_path() -> PathBuf {
    app_data_dir().join("api.token")
}
