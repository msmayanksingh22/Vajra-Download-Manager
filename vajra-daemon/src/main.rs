//! Vajra Daemon â€” API-first download manager service.
//!
//! Starts an axum HTTP server on 127.0.0.1:6277 (configurable).
//! All clients (UI, CLI, browser extension bridge) communicate through this API.

mod api;
mod rss_manager;
mod speed_history;

use std::{net::SocketAddr, sync::Arc, time::Instant};

use anyhow::Context;
use api::sse::SseBroadcaster;
use axum::http::StatusCode;
use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use vajra_engine::{
    db::{Database, HistoryEntry},
    download_task::TaskState,
    queue::{DownloadManager, DownloadManagerHandle, QueueSettings},
};
use vajra_protocol::DaemonConfig;

// â”€â”€â”€ Shared application state â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub struct AppState {
    pub manager: DownloadManagerHandle,
    pub database: Mutex<Database>,
    pub config: RwLock<DaemonConfig>,
    pub sse: SseBroadcaster,
    pub speed_tracker: Arc<speed_history::SpeedTracker>,
    pub started_at: Instant,
    /// Shutdown signal sender for graceful termination (e.g. post-queue ExitApp).
    pub shutdown_tx: tokio::sync::Mutex<Option<tokio::sync::broadcast::Sender<()>>>,
    pub ab_test: Arc<vajra_engine::ab_test::ExperimentManager>,
}

// â”€â”€â”€ Error type â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Not found: {0}")]
    NotFound(Uuid),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
}

impl axum::response::IntoResponse for DaemonError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match &self {
            DaemonError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            DaemonError::NotFound(id) => (
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Download {id} not found"),
            ),
            DaemonError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.clone(),
            ),
            DaemonError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, "db_error", e.to_string()),
        };
        (
            status,
            axum::Json(serde_json::json!({
                "error": { "code": code, "message": message }
            })),
        )
            .into_response()
    }
}

// â”€â”€â”€ Entry point â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vajra=info".parse()?)
                .add_directive("tower_http=debug".parse()?),
        )
        .init();

    // Windows: prevent system sleep while daemon is running
    #[cfg(target_os = "windows")]
    {
        // ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED
        // This tells Windows not to sleep while we have the daemon running.
        // We'll release this when the queue becomes idle (handled in queue.rs).
        use windows::Win32::System::Power::{
            SetThreadExecutionState, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
        };
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);
        }
        tracing::info!("Windows sleep prevention activated");
    }

    // Parse CLI args for portable mode
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--portable") {
        let exe_dir = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let portable_dir = exe_dir.join("vajra_data");
        std::env::set_var("VAJRA_DATA_DIR", &portable_dir);
        tracing::info!("Running in portable mode: {:?}", portable_dir);
    }

    // App data directory
    let app_dir = vajra_protocol::app_data_dir();
    std::fs::create_dir_all(&app_dir).context("Failed to create app data directory")?;

    // Load config
    let config = load_config(&app_dir);
    let port = config.listen_port;

    // Open database
    let database = Database::open(&vajra_protocol::db_path()).context("Failed to open database")?;
    let settings = database.load_settings().unwrap_or_default();

    // Build download manager
    let manager = DownloadManager::new(
        QueueSettings {
            max_concurrent: settings.max_concurrent_downloads.max(1) as usize,
            scheduler_enabled: settings.scheduler_enabled,
            scheduler_start_time: settings.scheduler_start_time.clone(),
            scheduler_stop_time: settings.scheduler_stop_time.clone(),
            fap_enabled: config.fap_enabled,
            fap_quota_bytes: config.fap_quota_mb * 1024 * 1024,
            fap_time_window_secs: config.fap_window_hours * 3600,
        },
        config.global_speed_limit_bps.unwrap_or(0),
    );

    let sse = SseBroadcaster::new();
    let speed_tracker = speed_history::SpeedTracker::new(600); // 10 minutes at 1 Hz

    // Use broadcast channel for shutdown - supports cloning so multiple callers can send signal
    let (shutdown_tx, mut serve_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Instantiate A/B Testing ExperimentManager
    let mut ab_mgr = vajra_engine::ab_test::ExperimentManager::new(settings.client_id.clone());
    ab_mgr.register_experiment("tcp_multiplexing_opt", 50);
    ab_mgr.register_experiment("adaptive_chunk_v2", 25);
    let ab_test = Arc::new(ab_mgr);

    let state = Arc::new(AppState {
        manager: manager.clone(),
        database: Mutex::new(database),
        config: RwLock::new(config),
        sse,
        speed_tracker: speed_tracker.clone(),
        started_at: Instant::now(),
        shutdown_tx: Mutex::new(Some(shutdown_tx)),
        ab_test,
    });

    let manager_clone = manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            let all = manager_clone.all_progress().await;
            let aggregate_speed: u64 = all.iter().map(|p| p.speed_bps).sum();
            speed_tracker.add_sample(aggregate_speed).await;
        }
    });

    let state_clone_vpn = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        let mut was_down = false;
        loop {
            interval.tick().await;

            let vpn_iface = {
                let config = state_clone_vpn.config.read().await;
                config.vpn_interface.clone()
            };

            if let Some(iface) = vpn_iface {
                let networks = sysinfo::Networks::new_with_refreshed_list();
                let is_up = networks.iter().any(|(name, _)| *name == iface);

                if !is_up && !was_down {
                    tracing::warn!("VPN kill switch activated! Interface {} is down. Pausing all active downloads.", iface);
                    let active = state_clone_vpn.manager.all_progress().await;
                    for p in active {
                        // Pause tasks that are currently downloading or allocating
                        let is_active = matches!(
                            p.state,
                            vajra_engine::download_task::TaskState::Downloading
                                | vajra_engine::download_task::TaskState::FetchingMeta
                                | vajra_engine::download_task::TaskState::Allocating
                        );
                        if is_active {
                            let _ = state_clone_vpn.manager.pause(p.id).await;
                        }
                    }
                    was_down = true;
                } else if is_up && was_down {
                    tracing::info!("VPN interface {} is back up.", iface);
                    was_down = false;
                }
            } else {
                was_down = false;
            }
        }
    });

    // Load all jobs from previous session
    let all_jobs = {
        let db = state.database.lock().await;
        db.load_all_jobs().unwrap_or_default()
    };

    let mut recovered_count = 0;
    for job in all_jobs {
        if let (Ok(id), Ok(request)) = (
            Uuid::parse_str(&job.id),
            serde_json::from_str::<vajra_engine::download_task::DownloadRequest>(&job.request_json),
        ) {
            let db_state = job.state.as_str();
            let is_active = matches!(
                db_state,
                "downloading" | "queued" | "fetching_meta" | "allocating" | "verifying"
            );

            if is_active {
                state.manager.add_with_id(id, request).await;
                recovered_count += 1;
            } else {
                let task_state = match db_state {
                    "paused" | "pausing" => TaskState::Paused,
                    "complete" | "completed" => TaskState::Completed,
                    "cancelled" => TaskState::Cancelled,
                    _ => TaskState::Failed,
                };

                let mut filename = request.filename.clone().unwrap_or_else(|| {
                    request
                        .url
                        .split('?')
                        .next()
                        .unwrap_or(&request.url)
                        .split('/')
                        .next_back()
                        .unwrap_or("download")
                        .to_string()
                });
                let mut dest_path = request
                    .dest_dir
                    .join(&filename)
                    .to_string_lossy()
                    .into_owned();
                let mut total_bytes = 0;
                let mut bytes_downloaded = 0;

                if task_state == TaskState::Completed {
                    let db = state.database.lock().await;
                    if let Ok(Some(hist)) = db.get_history_entry(&id.to_string()) {
                        filename = hist.filename;
                        dest_path = hist.dest_path;
                        total_bytes = hist.total_bytes;
                        bytes_downloaded = hist.total_bytes;
                    } else {
                        let path = request.dest_dir.join(&filename);
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            total_bytes = metadata.len();
                            bytes_downloaded = metadata.len();
                        }
                    }
                } else {
                    let state_path = request.dest_dir.join(format!(".{}.vajra.state", filename));
                    if let Ok(Some(saved)) = vajra_engine::state::DownloadState::load(&state_path) {
                        total_bytes = saved.total_bytes;
                        bytes_downloaded = saved.chunks.iter().map(|c| c.bytes_written).sum();
                    } else {
                        let path = request.dest_dir.join(&filename);
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            bytes_downloaded = metadata.len();
                        }
                    }
                }

                let restored_task = vajra_engine::download_task::DownloadTask::new_restored(
                    id,
                    request.clone(),
                    task_state,
                    bytes_downloaded,
                    total_bytes,
                    filename,
                    dest_path,
                    None,
                );
                state.manager.add_restored(id, request, restored_task).await;
            }
        }
    }
    if recovered_count > 0 {
        tracing::info!(
            "Recovered {} active download(s) from previous session",
            recovered_count
        );
    }

    // Spawn the progress persistence + SSE broadcast task
    tokio::spawn(progress_loop(Arc::clone(&state)));

    // Spawn the scheduler loop
    tokio::spawn(scheduler_loop(Arc::clone(&state)));

    // Spawn the sync loop
    tokio::spawn(sync_loop(Arc::clone(&state)));

    // Spawn the webhook loop
    tokio::spawn(webhook_loop(Arc::clone(&state)));

    // Start RSS Manager
    rss_manager::RssManager::start(Arc::clone(&state));

    // Build router
    let app = api::router::build(Arc::clone(&state)).await;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let tls_config = {
        let config = state.config.read().await;
        if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
            Some((cert_path.clone(), key_path.clone()))
        } else {
            None
        }
    };

    if let Some((cert, key)) = tls_config {
        tracing::info!("Vajra daemon listening on https://{}", addr);
        let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key)
            .await
            .context("Failed to load TLS certificates")?;

        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .context("Server error")?;
    } else {
        tracing::info!("Vajra daemon listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind to {addr}"))?;

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                // Wait for either OS signal (Ctrl+C / SIGTERM) OR internal shutdown signal
                // (e.g. post-queue ExitAction).
                tokio::select! {
                    _ = shutdown_signal() => {}
                    _ = serve_rx.recv() => {
                        tracing::info!("Internal shutdown signal received (post-queue action).");
                    }
                }
            })
            .await
            .context("Server error")?;
    }

    tracing::info!("Daemon shut down cleanly.");
    Ok(())
}

fn execute_post_queue_action(action: &vajra_protocol::PostQueueAction, state: &AppState) {
    tracing::info!("Executing post-queue action: {:?}", action);
    match action {
        vajra_protocol::PostQueueAction::None => {}
        vajra_protocol::PostQueueAction::ExitApp => {
            // FIX: Send graceful shutdown signal instead of std::process::exit(0).
            if let Ok(guard) = state.shutdown_tx.try_lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(());
                }
            }
        }
        vajra_protocol::PostQueueAction::Sleep => {
            #[cfg(target_os = "windows")]
            {
                let mut cmd = std::process::Command::new("rundll32.exe");
                cmd.args(["powrprof.dll,SetSuspendState", "0,1,0"]);
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                let _ = cmd.status();
            }
            #[cfg(target_os = "linux")]
            {
                let mut cmd = std::process::Command::new("systemctl");
                cmd.arg("suspend");
                let _ = cmd.status();
            }
        }
        vajra_protocol::PostQueueAction::Hibernate => {
            #[cfg(target_os = "windows")]
            {
                let mut cmd = std::process::Command::new("shutdown");
                cmd.args(["/h"]);
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                let _ = cmd.status();
            }
            #[cfg(target_os = "linux")]
            {
                let mut cmd = std::process::Command::new("systemctl");
                cmd.arg("hibernate");
                let _ = cmd.status();
            }
        }
        vajra_protocol::PostQueueAction::Shutdown => {
            #[cfg(target_os = "windows")]
            {
                let mut cmd = std::process::Command::new("shutdown");
                cmd.args(["/s", "/t", "10", "/f"]);
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                let _ = cmd.status();
            }
            #[cfg(target_os = "linux")]
            {
                let mut cmd = std::process::Command::new("shutdown");
                cmd.args(["-h", "now"]);
                let _ = cmd.status();
            }
        }
    }
}

// ——— Progress loop — persist state + broadcast SSE events ——————————————————————

async fn progress_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
    let mut last_states: std::collections::HashMap<uuid::Uuid, TaskState> =
        std::collections::HashMap::new();
    let mut queue_was_active = false;

    loop {
        interval.tick().await;
        let all = state.manager.all_progress().await;

        let has_active = all.iter().any(|p| {
            matches!(
                p.state,
                TaskState::Downloading
                    | TaskState::Allocating
                    | TaskState::FetchingMeta
                    | TaskState::Queued
            )
        });

        if has_active {
            queue_was_active = true;
        } else if queue_was_active {
            queue_was_active = false;
            let action = state.config.read().await.post_queue_action.clone();
            if action != vajra_protocol::PostQueueAction::None {
                execute_post_queue_action(&action, &state);
            }
        }

        let mut changed_tasks = Vec::new();
        // FIX: Scope the database lock to only the DB operations.
        // Drop it before the SSE broadcast so API endpoints are never blocked.
        {
            let db = state.database.lock().await;

            for p in &all {
                let prev_state = last_states.get(&p.id);
                let state_changed = prev_state.map(|s| *s != p.state).unwrap_or(true);

                if state_changed {
                    changed_tasks.push(p.clone());

                    let status_str = api::schema::state_str(&p.state);
                    let _ = db.update_job_state(&p.id.to_string(), status_str);

                    // Persist completed downloads to history
                    if p.state == TaskState::Completed {
                        let _ = db.insert_history(&HistoryEntry {
                            id: p.id.to_string(),
                            url: p.url.clone(),
                            filename: p.filename.clone(),
                            dest_path: p.dest_path.clone(),
                            total_bytes: p.total_bytes,
                            speed_avg_bps: p.speed_bps,
                            status: "complete".to_string(),
                            completed_at: Utc::now(),
                            tags: p.tags.clone(),
                        });
                    }
                }
            }
        } // ← DB lock released here

        // Phase 2: SSE broadcast (no lock held)
        for p in changed_tasks {
            last_states.insert(p.id, p.state.clone());

            // Play Asterisk completion sound & run AV scan
            if p.state == TaskState::Completed {
                let (play_sound, av_scan_path, av_scan_args) = {
                    let cfg = state.config.read().await;
                    (
                        cfg.sound_on_complete,
                        cfg.av_scan_path.clone(),
                        cfg.av_scan_args.clone(),
                    )
                };

                if play_sound {
                    #[cfg(target_os = "windows")]
                    {
                        use windows::Win32::{
                            System::Diagnostics::Debug::MessageBeep,
                            UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
                        };
                        unsafe {
                            let _ = MessageBeep(MESSAGEBOX_STYLE(0x00000040));
                            // MB_ICONASTERISK
                        }
                    }
                }

                if let Some(av_path) = av_scan_path {
                    if !av_path.is_empty() {
                        let file_path = p.dest_path.clone();
                        tokio::spawn(async move {
                            let mut cmd = tokio::process::Command::new(av_path);
                            cmd.stdin(std::process::Stdio::null());
                            let mut has_file_placeholder = false;
                            for arg in &av_scan_args {
                                if arg == "%file%" {
                                    cmd.arg(&file_path);
                                    has_file_placeholder = true;
                                } else {
                                    cmd.arg(arg);
                                }
                            }
                            if !has_file_placeholder {
                                cmd.arg(&file_path);
                            }

                            #[cfg(target_os = "windows")]
                            {
                                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                            }

                            match cmd.output().await {
                                Ok(output) => {
                                    tracing::info!(
                                        "AV Scan completed. Status success: {}",
                                        output.status.success()
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("Failed to launch AV scanner: {}", e);
                                }
                            }
                        });
                    }
                }
            }

            // Broadcast state changes for terminal or downloading states
            if matches!(
                p.state,
                TaskState::Completed
                    | TaskState::Failed
                    | TaskState::Cancelled
                    | TaskState::Paused
                    | TaskState::Downloading
            ) {
                state.sse.send(vajra_protocol::DaemonEvent::StateChange {
                    id: p.id,
                    status: api::schema::state_to_status(&p.state),
                    output_path: if p.dest_path.is_empty() {
                        None
                    } else {
                        Some(p.dest_path.clone())
                    },
                    error: p.error.clone(),
                });
            }
        }

        let mut batch_items = Vec::new();
        for p in &all {
            if matches!(
                p.state,
                TaskState::Downloading | TaskState::Allocating | TaskState::FetchingMeta
            ) {
                batch_items.push(vajra_protocol::BatchProgressItem {
                    download_id: p.id,
                    url: p.url.clone(),
                    filename: p.filename.clone(),
                    total_bytes: if p.total_bytes > 0 {
                        Some(p.total_bytes)
                    } else {
                        None
                    },
                    downloaded_bytes: p.bytes_downloaded,
                    speed_bps: p.speed_bps,
                    eta_seconds: if p.eta_secs > 0 {
                        Some(p.eta_secs)
                    } else {
                        None
                    },
                    status: api::schema::state_to_status(&p.state),
                    resume_supported: p.resume_supported,
                    segments: p.segments.clone(),
                    error: p.error.clone(),
                    speed_limit_bps: p.speed_limit_bps,
                });
            }
        }

        if !batch_items.is_empty() {
            state.sse.send(vajra_protocol::DaemonEvent::BatchProgress {
                downloads: batch_items,
            });
        }
    }
}
// â”€â”€â”€ Scheduler loop â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn scheduler_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    let mut last_checked_minute = -1;

    loop {
        interval.tick().await;

        let config = state.config.read().await;
        let enabled = config.scheduler_enabled;
        let start_time = config.scheduler_start_time.clone();
        let stop_time = config.scheduler_stop_time.clone();
        drop(config);

        if !enabled {
            continue;
        }

        let now = chrono::Local::now();
        let current_minute = now.format("%M").to_string().parse::<i32>().unwrap_or(0);
        let current_time_str = now.format("%H:%M").to_string();

        if current_minute != last_checked_minute {
            last_checked_minute = current_minute;

            if let Some(st) = &start_time {
                if current_time_str == *st {
                    tracing::info!("Scheduler triggered start at {}", current_time_str);
                    state.manager.resume_all().await;
                }
            }

            if let Some(st) = &stop_time {
                if current_time_str == *st {
                    tracing::info!("Scheduler triggered stop at {}", current_time_str);
                    state.manager.pause_all().await;
                }
            }
        }
    }
}

// â”€â”€â”€ Sync loop â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn sync_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        interval.tick().await;

        let all_progress = state.manager.all_progress().await;
        for p in all_progress {
            if p.queue_type == vajra_engine::download_task::QueueType::Synchronization
                && (p.state == TaskState::Completed || p.state == TaskState::Failed)
            {
                let db = state.database.lock().await;
                let job = db.load_job(&p.id.to_string()).ok().flatten();
                drop(db);

                if let Some(job) = job {
                    let now = chrono::Utc::now().timestamp();
                    let updated_at = job.updated_at.timestamp();
                    let interval_secs = p.sync_interval_secs.max(60); // minimum 60 seconds

                    if now - updated_at >= (interval_secs as i64) {
                        tracing::info!(
                            "SyncQueue: Checking for updates for '{}' (ID: {})",
                            p.filename,
                            p.id
                        );

                        // Perform HEAD request to check for modifications
                        let client = reqwest::Client::new();
                        if let Ok(resp) = client.head(&p.url).send().await {
                            let mut modified = true;

                            let new_len = resp
                                .headers()
                                .get(reqwest::header::CONTENT_LENGTH)
                                .and_then(|v| v.to_str().ok())
                                .and_then(|v| v.parse::<u64>().ok());

                            if let Some(new_len) = new_len {
                                if new_len == p.total_bytes && p.state == TaskState::Completed {
                                    tracing::info!(
                                        "SyncQueue: File size matches, assuming unmodified"
                                    );
                                    modified = false;
                                }
                            }

                            if modified {
                                tracing::info!(
                                    "SyncQueue: Modifications detected, resuming task {}",
                                    p.id
                                );
                                let _ = std::fs::remove_file(&p.dest_path);
                                {
                                    let db = state.database.lock().await;
                                    let mut job_record = job.clone();
                                    job_record.updated_at = chrono::Utc::now();
                                    let _ = db.upsert_job(&job_record);
                                }
                                state.manager.resume(p.id).await;
                            } else {
                                let db = state.database.lock().await;
                                let mut job_record = job.clone();
                                job_record.updated_at = chrono::Utc::now();
                                let _ = db.upsert_job(&job_record);
                            }
                        }
                    }
                }
            }
        }
    }
}

// â”€â”€â”€ Config loader â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn load_config(_app_dir: &std::path::Path) -> DaemonConfig {
    let path = vajra_protocol::config_path();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<DaemonConfig>(&raw) {
            tracing::info!("Loaded config from {}", path.display());
            return cfg;
        }
    }
    let cfg = DaemonConfig::default();
    // Write defaults so users can see and edit them
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(&path, json);
    }
    cfg
}

// â”€â”€â”€ Graceful shutdown â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutdown signal received, stopping gracefully...");
}

async fn webhook_loop(state: Arc<AppState>) {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let mut rx = state.sse.subscribe();
    let client = reqwest::Client::new();
    while let Ok(msg) = rx.recv().await {
        let (is_terminal, event_name) = match &*msg {
            vajra_protocol::DaemonEvent::StateChange { status, .. } => match status {
                vajra_protocol::DownloadStatus::Completed => (true, "DownloadCompleted"),
                vajra_protocol::DownloadStatus::Failed => (true, "DownloadFailed"),
                _ => (false, ""),
            },
            _ => (false, ""),
        };

        if is_terminal {
            let config = state.config.read().await;
            let webhooks = config.webhooks.clone();
            let secret = config.webhook_secret.clone();
            drop(config);

            if webhooks.is_empty() {
                continue;
            }

            let payload = serde_json::json!({
                "event": event_name,
                "data": &*msg
            });
            let payload_str = payload.to_string();

            let mut signature_header = None;
            if let Some(sec) = secret {
                if !sec.is_empty() {
                    if let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(sec.as_bytes()) {
                        mac.update(payload_str.as_bytes());
                        let result = mac.finalize();
                        let hex_sig = hex::encode(result.into_bytes());
                        signature_header = Some(hex_sig);
                    }
                }
            }

            for url in webhooks {
                let client = client.clone();
                let payload_str = payload_str.clone();
                let sig = signature_header.clone();
                tokio::spawn(async move {
                    let mut req = client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .body(payload_str);

                    if let Some(s) = sig {
                        req = req.header("X-Vajra-Signature", format!("sha256={}", s));
                    }

                    if let Err(e) = req.send().await {
                        tracing::warn!("Failed to send webhook to {}: {}", url, e);
                    } else {
                        tracing::info!("Sent webhook to {}", url);
                    }
                });
            }
        }
    }
}
