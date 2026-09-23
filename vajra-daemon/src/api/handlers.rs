//! REST API handlers — one async fn per endpoint.

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Json,
    },
};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;
use vajra_engine::download_task::DownloadRequest;
use vajra_protocol::{
    AddDownloadRequest, AddDownloadResponse, BulkAction, BulkActionFailure, BulkActionRequest,
    BulkActionResponse, DownloadAction, DownloadInfo, DownloadList, InspectRequest,
    InspectResponse, PatchDownloadRequest, StatsResponse,
};

use crate::{
    api::{
        schema::{progress_to_info, state_str},
        sse::to_sse_event,
    },
    AppState, DaemonError,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

type Result<T> = std::result::Result<T, DaemonError>;

// ─── GET /health ─────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/health", responses((status = 200, description = "OK")))]
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "api_version": vajra_protocol::API_VERSION,
        "daemon_version": vajra_protocol::DAEMON_VERSION,
    }))
}

// ─── POST /api/v1/downloads ───────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/downloads", request_body = AddDownloadRequest, responses((status = 201, description = "Download Added")))]
pub async fn add_download(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddDownloadRequest>,
) -> Result<impl IntoResponse> {
    // 1. Rate limit check
    if !AppState::check_rate_limit(
        &state.add_download_limiter,
        60,
        std::time::Duration::from_secs(10),
    )
    .await
    {
        return Err(DaemonError::RateLimited(
            "Download creation rate limit exceeded, please slow down".into(),
        ));
    }

    // 2. Validate URL length and scheme
    let trimmed_url = body.url.trim();
    if trimmed_url.len() > 4096 {
        return Err(DaemonError::BadRequest(
            "URL exceeds maximum length of 4096 characters".into(),
        ));
    }

    let lower_url = trimmed_url.to_lowercase();
    if !lower_url.starts_with("http://")
        && !lower_url.starts_with("https://")
        && !lower_url.starts_with("magnet:")
    {
        return Err(DaemonError::BadRequest(
            "Only HTTP(S) and magnet: URLs are supported".into(),
        ));
    }

    // 3. Validate filename length if provided
    if let Some(ref fname) = body.filename {
        if fname.len() > 255 {
            return Err(DaemonError::BadRequest(
                "Filename exceeds maximum length of 255 characters".into(),
            ));
        }
    }

    // 4. Validate tags
    if let Some(ref tags) = body.tags {
        if tags.len() > 50 {
            return Err(DaemonError::BadRequest("Too many tags (maximum 50)".into()));
        }
        for tag in tags {
            if tag.len() > 100 {
                return Err(DaemonError::BadRequest(
                    "Tag exceeds maximum length of 100 characters".into(),
                ));
            }
        }
    }

    // 5. Validate custom headers count
    if body.headers.len() > 50 {
        return Err(DaemonError::BadRequest(
            "Too many custom headers (maximum 50)".into(),
        ));
    }

    let config = state.config.read().await;
    // BUG-21: clamp caller-supplied values to sane bounds.
    // max_connections: 1–32 (prevents u32::MAX connection attempts).
    // speed_limit_bps: 0 (unlimited) or up to 10 Gbps.
    let max_connections = body
        .max_connections
        .unwrap_or(config.default_max_connections as u32)
        .clamp(1, 32);
    let speed_limit = body.speed_limit_bps.unwrap_or(0).min(1_250_000_000); // cap at 10 Gbps

    // Auto-categorize logic disabled based on user request.
    // Default to system Downloads folder, then config fallback, unless explicitly provided.
    let output_dir = if let Some(dir) = body.output_dir.as_deref() {
        match vajra_protocol::path_security::validate_output_dir(std::path::Path::new(dir)) {
            Ok(p) => p,
            Err(e) => {
                return Err(DaemonError::BadRequest(format!("Invalid output_dir: {e}")));
            }
        }
    } else {
        dirs_next::download_dir().unwrap_or_else(|| PathBuf::from(&config.default_output_dir))
    };

    // We keep config around longer so we can copy AV properties
    // drop(config);

    // Extract well-known headers from the generic map
    let cookie = body
        .headers
        .get("Cookie")
        .or_else(|| body.headers.get("cookie"))
        .cloned();
    let referrer = body
        .headers
        .get("Referer")
        .or_else(|| body.headers.get("referer"))
        .cloned();
    let user_agent = body
        .headers
        .get("User-Agent")
        .or_else(|| body.headers.get("user-agent"))
        .cloned();

    // Check vault for credentials
    let mut authorization = None;
    if let Ok(parsed_url) = url::Url::parse(&body.url) {
        if let Some(domain) = parsed_url.host_str() {
            let db = state.database.lock().await;
            if let Ok(Some(cred)) = db.get_credential_by_domain(domain) {
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("{}:{}", cred.username, cred.password),
                );
                authorization = Some(format!("Basic {}", encoded));
            }
        }
    }

    let proxy = config.proxy.url.clone();
    let proxies = config.proxy.urls.clone();

    // Determine local_address from bind_interface
    let mut local_address = None;
    if let Some(iface_name) = &config.bind_interface {
        let networks = sysinfo::Networks::new_with_refreshed_list();
        if let Some((_, network)) = networks.iter().find(|(name, _)| *name == iface_name) {
            if let Some(ip) = network.ip_networks().first() {
                local_address = Some(ip.addr);
            }
        }
    }

    let mut target_url = body.url.clone();
    if vajra_engine::cloud::is_cloud_link(&target_url) {
        tracing::info!(
            "Consumer Cloud share link detected: '{}'. Attempting auto-translation...",
            target_url
        );
        match vajra_engine::cloud::translate_cloud_link(&target_url).await {
            Ok(translated) => {
                tracing::info!(
                    "Successfully translated cloud link to direct download: '{}'",
                    translated
                );
                target_url = translated;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to translate cloud link: {}. Downloading original URL.",
                    e
                );
            }
        }
    }

    let tcp_multiplexing_opt = state.ab_test.is_enabled("tcp_multiplexing_opt");
    let adaptive_chunk_v2 = state.ab_test.is_enabled("adaptive_chunk_v2");

    let request = DownloadRequest {
        url: target_url,
        mirrors: vec![],
        dest_dir: output_dir,
        filename: body
            .filename
            .as_deref()
            .map(vajra_protocol::sanitize_filename),
        timeout_secs: None,
        connect_timeout_secs: None,
        max_connections,
        speed_limit,
        delete_on_failure: false,
        use_http3: body.use_http3 || config.default_use_http3,
        referrer,
        cookie_header: cookie,
        user_agent,
        authorization,
        proxy,
        proxies,
        local_address,
        tcp_multiplexing_opt,
        adaptive_chunk_v2,
        use_ytdlp: body.use_ytdlp,
        ytdlp_format: body.ytdlp_format.clone(),
        ytdlp_subtitles: body.ytdlp_subtitles,
        ytdlp_playlist: body.ytdlp_playlist,
        throttle: None,
        expected_hash: body.expected_hash.clone(),
        auto_extract: body.auto_extract || config.auto_extract,
        post_processing_script: config.post_process_script.clone(),
        av_scan_path: config.av_scan_path.clone(),
        av_scan_args: config.av_scan_args.clone(),
        schedule_at: body.schedule_at,
        queue_type: body.queue_type.clone().unwrap_or_default(),
        sync_interval_secs: body.sync_interval_secs.unwrap_or(3600),
        priority: vajra_protocol::Priority::Normal,
        tags: body.tags.clone().unwrap_or_default(),
        daemon_config: Some((*config).clone()),
        multiplexer_options: None,
        duplicate_action: body.duplicate_action,
    };

    drop(config);
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Persist job to DB
    {
        let db = state.database.lock().await;
        db.upsert_job(&vajra_engine::db::JobRecord {
            id: id.to_string(),
            request_json: serde_json::to_string(&request.to_redacted()).unwrap_or_default(),
            state: "queued".to_string(),
            created_at: now,
            updated_at: now,
        })?;
    }

    state.manager.add_with_id(id, request).await;

    // Notify SSE subscribers
    let filename = body.filename.unwrap_or_else(|| {
        let base = body.url.split('#').next().unwrap_or(&body.url);
        let base = base.split('?').next().unwrap_or(base);
        base.split('/')
            .next_back()
            .filter(|s| !s.is_empty())
            .unwrap_or("download")
            .to_string()
    });
    state.sse.send(vajra_protocol::DaemonEvent::Added {
        id,
        url: body.url.clone(),
        filename: filename.clone(),
    });

    Ok((
        StatusCode::CREATED,
        Json(AddDownloadResponse {
            id,
            status: "queued".to_string(),
            url: body.url,
            filename: Some(filename),
            created_at: now.timestamp(),
        }),
    ))
}

// ─── GET /api/v1/downloads ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}
fn default_limit() -> usize {
    50
}

#[utoipa::path(get, path = "/api/v1/downloads", responses((status = 200, description = "List of downloads")))]
pub async fn list_downloads(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse> {
    let all = state.manager.all_progress().await;
    let items: Vec<DownloadInfo> = all
        .iter()
        .filter(|p| {
            params
                .status
                .as_deref()
                .map(|s| state_str(&p.state) == s || s == "all")
                .unwrap_or(true)
        })
        .map(progress_to_info)
        .collect();

    let limit = params.limit.clamp(1, 500);
    let total = items.len();
    let paged: Vec<DownloadInfo> = items.into_iter().skip(params.offset).take(limit).collect();

    Ok(Json(DownloadList {
        total,
        limit,
        offset: params.offset,
        items: paged,
    }))
}

// ─── GET /api/v1/downloads/:id ────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/downloads/{id}", responses((status = 200, description = "Download info")))]
pub async fn get_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let p = state
        .manager
        .progress(id)
        .await
        .ok_or(DaemonError::NotFound(id))?;
    let response = progress_to_info(&p);
    Ok(Json(response))
}

// ─── PATCH /api/v1/downloads/:id ─────────────────────────────────────────────

#[utoipa::path(patch, path = "/api/v1/downloads/{id}", request_body = PatchDownloadRequest, responses((status = 200, description = "Download patched")))]
pub async fn patch_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchDownloadRequest>,
) -> Result<impl IntoResponse> {
    // 1.2 Handle Filename change (rename)
    if let Some(new_filename) = &body.filename {
        let clean = vajra_protocol::sanitize_filename(new_filename);
        if clean.is_empty() || (clean == "download" && new_filename.trim().is_empty()) {
            return Err(DaemonError::BadRequest(
                "Invalid filename: cannot be empty".into(),
            ));
        }
        let new_filename = &clean;

        if state
            .manager
            .update_filename(id, new_filename.to_string())
            .await
            .is_err()
        {
            return Err(DaemonError::NotFound(id));
        }

        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                request.filename = Some(new_filename.to_string());
                if let Ok(new_json) = serde_json::to_string(&request.to_redacted()) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        }

        if let Ok(Some(mut hist)) = db.get_history_entry(&id.to_string()) {
            hist.filename = new_filename.to_string();
            let old_dest_path = std::path::Path::new(&hist.dest_path);
            if let Some(parent) = old_dest_path.parent() {
                hist.dest_path = parent.join(new_filename).to_string_lossy().to_string();
            }
            let _ = db.insert_history(&hist);
        }
    }

    // 1. Handle URL change (refresh link)
    if let Some(new_url) = &body.url {
        let trimmed_new_url = new_url.trim();
        if trimmed_new_url.len() > 4096 {
            return Err(DaemonError::BadRequest(
                "URL exceeds maximum length of 4096 characters".into(),
            ));
        }
        let lower_new_url = trimmed_new_url.to_lowercase();
        if !lower_new_url.starts_with("http://")
            && !lower_new_url.starts_with("https://")
            && !lower_new_url.starts_with("magnet:")
        {
            return Err(DaemonError::BadRequest(
                "Only HTTP(S) and magnet: URLs are supported".into(),
            ));
        }

        if state.manager.update_url(id, new_url.clone()).await.is_err() {
            return Err(DaemonError::NotFound(id));
        }

        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                request.url = new_url.clone();
                if let Ok(new_json) = serde_json::to_string(&request.to_redacted()) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        }
    }

    // 2. Handle Settings change (speed limit, max connections)
    if body.speed_limit_bps.is_some() || body.max_connections.is_some() {
        let speed_limit = body
            .speed_limit_bps
            .map(|opt| opt.unwrap_or(0).min(1_250_000_000));
        let max_connections = body.max_connections.map(|c| c.clamp(1, 32));

        if state
            .manager
            .update_download_settings(id, speed_limit, max_connections)
            .await
            .is_err()
        {
            return Err(DaemonError::NotFound(id));
        }

        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                if let Some(lim) = speed_limit {
                    request.speed_limit = lim;
                }
                if let Some(conn) = max_connections {
                    request.max_connections = conn;
                }
                if let Ok(new_json) = serde_json::to_string(&request.to_redacted()) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        }
    }

    // 2.5 Handle Tags change
    if let Some(new_tags) = &body.tags {
        if new_tags.len() > 50 {
            return Err(DaemonError::BadRequest("Too many tags (maximum 50)".into()));
        }
        for t in new_tags {
            if t.len() > 100 {
                return Err(DaemonError::BadRequest(
                    "Tag length exceeds 100 characters".into(),
                ));
            }
        }

        if state
            .manager
            .update_tags(id, new_tags.clone())
            .await
            .is_err()
        {
            // Might not be active anymore, but we can still update the DB
        }
        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                request.tags = new_tags.clone();
                if let Ok(new_json) = serde_json::to_string(&request.to_redacted()) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        } else if let Ok(Some(mut hist)) = db.get_history_entry(&id.to_string()) {
            hist.tags = new_tags.clone();
            let _ = db.insert_history(&hist);
        }
    }

    // 3. Handle Lifecycle action
    if let Some(action) = &body.action {
        match action {
            DownloadAction::Pause => state.manager.pause(id).await,
            DownloadAction::Resume | DownloadAction::Retry => {
                if let Some(p) = state.manager.progress(id).await {
                    if let Ok(parsed) = url::Url::parse(&p.url) {
                        if let Some(domain) = parsed.host_str() {
                            let db = state.database.lock().await;
                            if let Ok(Some(cred)) = db.get_credential_by_domain(domain) {
                                let encoded = base64::Engine::encode(
                                    &base64::engine::general_purpose::STANDARD,
                                    format!("{}:{}", cred.username, cred.password),
                                );
                                state
                                    .manager
                                    .update_authorization(id, Some(format!("Basic {}", encoded)))
                                    .await;
                            }
                        }
                    }
                }
                state.manager.resume(id).await;
            }
            DownloadAction::Cancel => {
                state.manager.cancel(id).await;
                state
                    .database
                    .lock()
                    .await
                    .update_job_state(&id.to_string(), "cancelled")?;
            }
        }
        let status = match action {
            DownloadAction::Pause => vajra_protocol::DownloadStatus::Paused,
            DownloadAction::Resume | DownloadAction::Retry => {
                vajra_protocol::DownloadStatus::Connecting
            }
            DownloadAction::Cancel => vajra_protocol::DownloadStatus::Failed,
        };
        state.sse.send(vajra_protocol::DaemonEvent::StateChange {
            id,
            status,
            output_path: None,
            error: None,
        });
    }
    Ok(Json(serde_json::json!({ "id": id, "ok": true })))
}

// ─── DELETE /api/v1/downloads/:id ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteParams {
    #[serde(default)]
    pub delete_file: bool,
}

pub(crate) async fn delete_download_internal(
    state: &AppState,
    id: Uuid,
    delete_file: bool,
) -> Result<()> {
    let progress = state.manager.progress(id).await;

    // Check existence across manager and database
    let mut file_path_to_delete = None;
    let mut state_path_to_delete = None;
    let mut exists_in_db = false;

    {
        let db = state.database.lock().await;
        if let Ok(Some(hist)) = db.get_history_entry(&id.to_string()) {
            exists_in_db = true;
            if delete_file && !hist.dest_path.is_empty() {
                let file_path = std::path::PathBuf::from(&hist.dest_path);
                file_path_to_delete = Some(file_path);
                let filename = std::path::Path::new(&hist.dest_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if let Some(parent) = std::path::Path::new(&hist.dest_path).parent() {
                    state_path_to_delete = Some(parent.join(format!(".{}.vajra.state", filename)));
                }
            }
        } else if let Ok(Some(job)) = db.get_job(&id.to_string()) {
            exists_in_db = true;
            if delete_file {
                if let Ok(request) = serde_json::from_str::<
                    vajra_engine::download_task::DownloadRequest,
                >(&job.request_json)
                {
                    let filename = request.filename.unwrap_or_else(|| {
                        request
                            .url
                            .split('?')
                            .next()
                            .unwrap_or("")
                            .split('/')
                            .next_back()
                            .unwrap_or("")
                            .to_string()
                    });
                    if !filename.is_empty() {
                        let file_path = request.dest_dir.join(&filename);
                        let state_path =
                            request.dest_dir.join(format!(".{}.vajra.state", filename));
                        file_path_to_delete = Some(file_path);
                        state_path_to_delete = Some(state_path);
                    }
                }
            }
        }
    }

    if progress.is_none() && !exists_in_db {
        return Err(DaemonError::NotFound(id));
    }

    if delete_file && file_path_to_delete.is_none() {
        if let Some(p) = &progress {
            if !p.dest_path.is_empty() {
                let file_path = std::path::PathBuf::from(&p.dest_path);
                file_path_to_delete = Some(file_path);
                let filename = std::path::Path::new(&p.dest_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if let Some(parent) = std::path::Path::new(&p.dest_path).parent() {
                    state_path_to_delete = Some(parent.join(format!(".{}.vajra.state", filename)));
                }
            }
        }
    }

    // 1. Cancel active task first to release any open file handles
    state.manager.cancel(id).await;

    // 2. Physical file deletion with bounded retry:
    // If deletion fails with an I/O error, report it before mutating database records!
    if delete_file {
        if let Some(path) = file_path_to_delete {
            if path.exists() {
                let mut last_err = None;
                for _ in 0..2 {
                    let result = if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    match result {
                        Ok(()) => {
                            last_err = None;
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            last_err = None;
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                        }
                    }
                }
                if let Some(e) = last_err {
                    return Err(DaemonError::Io(e));
                }
            }
        }
        if let Some(path) = state_path_to_delete {
            if path.exists() {
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
            }
        }
    }

    // 3. Delete records from database after successful file cleanup (or if file deletion was not requested)
    {
        let db = state.database.lock().await;
        let _ = db.delete_job(&id.to_string());
        let _ = db.delete_history_entry(&id.to_string());
    }

    state.sse.send(vajra_protocol::DaemonEvent::Removed { id });
    Ok(())
}

pub(crate) async fn clear_completed_internal(
    state: &AppState,
    id: Uuid,
) -> std::result::Result<(), (String, String)> {
    let progress = state.manager.progress(id).await;
    let Some(p) = progress else {
        return Err(("not_found".to_string(), "Download not found".to_string()));
    };

    if p.state != vajra_engine::download_task::TaskState::Completed {
        return Err((
            "invalid_state".to_string(),
            format!(
                "Download cannot be cleared because it is in {:?} state",
                p.state
            ),
        ));
    }

    // Remove from active manager memory
    state.manager.cancel(id).await;

    // Remove from active jobs DB table, but STRICTLY PRESERVE history DB table!
    {
        let db = state.database.lock().await;
        let _ = db.delete_job(&id.to_string());
    }

    state.sse.send(vajra_protocol::DaemonEvent::Removed { id });
    Ok(())
}

#[utoipa::path(delete, path = "/api/v1/downloads/{id}", responses((status = 200, description = "Download deleted")))]
pub async fn delete_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteParams>,
) -> Result<impl IntoResponse> {
    delete_download_internal(&state, id, params.delete_file).await?;
    Ok(Json(serde_json::json!({ "id": id, "ok": true })))
}

fn record_delete_result(
    id: Uuid,
    res: Result<()>,
    succeeded: &mut Vec<Uuid>,
    failed: &mut Vec<BulkActionFailure>,
) {
    match res {
        Ok(()) => succeeded.push(id),
        Err(DaemonError::NotFound(_)) => failed.push(BulkActionFailure {
            id,
            code: "not_found".into(),
            message: "Download not found".into(),
        }),
        Err(DaemonError::Io(e)) => failed.push(BulkActionFailure {
            id,
            code: "io_error".into(),
            message: e.to_string(),
        }),
        Err(e) => failed.push(BulkActionFailure {
            id,
            code: "db_error".into(),
            message: e.to_string(),
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/downloads/bulk-action",
    request_body = BulkActionRequest,
    responses(
        (status = 200, description = "Bulk action performed (may contain per-item partial failures)", body = BulkActionResponse),
        (status = 400, description = "Invalid request: batch size >500, contradictory 'all' and 'ids', or empty request"),
        (status = 401, description = "Unauthorized (missing or invalid bearer token)"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn bulk_action(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkActionRequest>,
) -> Result<Json<BulkActionResponse>> {
    // 1. Enforce ID limit of 500 on the submitted request BEFORE deduplication
    if body.ids.len() > 500 {
        return Err(DaemonError::BadRequest(
            "Bulk action batch size exceeds maximum limit of 500 items".into(),
        ));
    }

    // 2. Reject all=true combined with a non-empty list of IDs
    if body.all && !body.ids.is_empty() {
        return Err(DaemonError::BadRequest(
            "Cannot specify both 'all: true' and a non-empty list of IDs".into(),
        ));
    }

    // 3. Reject empty request (neither all: true nor non-empty ids)
    if !body.all && body.ids.is_empty() {
        return Err(DaemonError::BadRequest(
            "Must specify either 'all: true' or a non-empty list of IDs".into(),
        ));
    }

    // 4. Deduplicate IDs
    let mut unique_ids = body.ids.clone();
    unique_ids.sort();
    unique_ids.dedup();

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    match body.action {
        BulkAction::Pause => {
            if body.all {
                let all = state.manager.all_progress().await;
                for p in all {
                    if let Ok(()) = state.manager.pause_task(p.id).await {
                        succeeded.push(p.id);
                    }
                }
            } else {
                for id in unique_ids {
                    match state.manager.pause_task(id).await {
                        Ok(()) => succeeded.push(id),
                        Err(vajra_engine::QueueActionError::NotFound) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "not_found".into(),
                                message: "Download not found".into(),
                            });
                        }
                        Err(vajra_engine::QueueActionError::InvalidState(msg)) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "invalid_state".into(),
                                message: msg,
                            });
                        }
                    }
                }
            }
        }
        BulkAction::Resume => {
            if body.all {
                let all = state.manager.all_progress().await;
                for p in all {
                    if matches!(
                        p.state,
                        vajra_engine::download_task::TaskState::Paused
                            | vajra_engine::download_task::TaskState::Failed
                            | vajra_engine::download_task::TaskState::Cancelled
                    ) {
                        if let Ok(()) = state.manager.resume_task(p.id).await {
                            succeeded.push(p.id);
                        }
                    }
                }
            } else {
                for id in unique_ids {
                    match state.manager.resume_task(id).await {
                        Ok(()) => succeeded.push(id),
                        Err(vajra_engine::QueueActionError::NotFound) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "not_found".into(),
                                message: "Download not found".into(),
                            });
                        }
                        Err(vajra_engine::QueueActionError::InvalidState(msg)) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "invalid_state".into(),
                                message: msg,
                            });
                        }
                    }
                }
            }
        }
        BulkAction::Retry => {
            if body.all {
                let all = state.manager.all_progress().await;
                for p in all {
                    if matches!(
                        p.state,
                        vajra_engine::download_task::TaskState::Failed
                            | vajra_engine::download_task::TaskState::Cancelled
                    ) {
                        if let Ok(()) = state.manager.retry_task(p.id).await {
                            succeeded.push(p.id);
                        }
                    }
                }
            } else {
                for id in unique_ids {
                    match state.manager.retry_task(id).await {
                        Ok(()) => succeeded.push(id),
                        Err(vajra_engine::QueueActionError::NotFound) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "not_found".into(),
                                message: "Download not found".into(),
                            });
                        }
                        Err(vajra_engine::QueueActionError::InvalidState(msg)) => {
                            failed.push(BulkActionFailure {
                                id,
                                code: "invalid_state".into(),
                                message: msg,
                            });
                        }
                    }
                }
            }
        }
        BulkAction::ClearCompleted => {
            if body.all {
                let all = state.manager.all_progress().await;
                for p in all {
                    if p.state == vajra_engine::download_task::TaskState::Completed {
                        if let Ok(()) = clear_completed_internal(&state, p.id).await {
                            succeeded.push(p.id);
                        }
                    }
                }
            } else {
                for id in unique_ids {
                    match clear_completed_internal(&state, id).await {
                        Ok(()) => succeeded.push(id),
                        Err((code, message)) => {
                            failed.push(BulkActionFailure { id, code, message });
                        }
                    }
                }
            }
        }
        BulkAction::Delete => {
            let target_ids = if body.all {
                let all = state.manager.all_progress().await;
                all.into_iter().map(|p| p.id).collect()
            } else {
                unique_ids
            };

            let state_arc = Arc::clone(&state);
            let delete_file = body.delete_file;
            let mut set = tokio::task::JoinSet::new();

            for id in target_ids {
                let st = Arc::clone(&state_arc);
                while set.len() >= 16 {
                    if let Some(Ok((id, res))) = set.join_next().await {
                        record_delete_result(id, res, &mut succeeded, &mut failed);
                    }
                }
                set.spawn(async move {
                    let res = delete_download_internal(&st, id, delete_file).await;
                    (id, res)
                });
            }

            while let Some(Ok((id, res))) = set.join_next().await {
                record_delete_result(id, res, &mut succeeded, &mut failed);
            }
        }
    }

    let total = succeeded.len() + failed.len();
    Ok(Json(BulkActionResponse {
        total,
        succeeded,
        failed,
    }))
}

// ─── GET /api/v1/downloads/:id/events (per-download SSE) ─────────────────────

pub async fn download_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, std::convert::Infallible>>> {
    let rx = state.sse.subscribe();
    let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), move |msg| {
        let ev = msg.ok()?;
        // Only forward events that belong to this download
        let belongs = match ev.as_ref() {
            vajra_protocol::DaemonEvent::Progress {
                download_id: eid, ..
            } => *eid == id,
            vajra_protocol::DaemonEvent::StateChange { id: eid, .. } => *eid == id,
            vajra_protocol::DaemonEvent::HashResult { id: eid, .. } => *eid == id,
            _ => false,
        };
        if !belongs {
            return None;
        }
        Some(Ok(
            to_sse_event(&ev).unwrap_or_else(|_| Event::default().comment("error"))
        ))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ─── GET /api/v1/events (global SSE) ─────────────────────────────────────────

pub async fn global_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, std::convert::Infallible>>> {
    let rx = state.sse.subscribe();
    let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), |msg| {
        let ev = msg.ok()?;
        Some(Ok(
            to_sse_event(&ev).unwrap_or_else(|_| Event::default().comment("error"))
        ))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ─── GET /api/v1/ws (global WebSocket) ───────────────────────────────────────

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.sse.subscribe();
    use futures_util::StreamExt;
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // we serialize the same DaemonEvent as JSON
            if let Ok(json) = serde_json::to_string(&*msg) {
                if futures_util::SinkExt::send(&mut sender, Message::Text(json))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

// ─── POST /api/v1/inspect ────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/inspect",
    request_body = InspectRequest,
    responses(
        (status = 200, description = "URL inspection result", body = InspectResponse),
        (status = 400, description = "Bad request", body = ApiErrorResponse),
        (status = 429, description = "Rate limited", body = ApiErrorResponse)
    )
)]
pub async fn inspect_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InspectRequest>,
) -> Result<impl IntoResponse> {
    // 1. Rate limit check
    if !AppState::check_rate_limit(
        &state.inspect_limiter,
        30,
        std::time::Duration::from_secs(10),
    )
    .await
    {
        return Err(DaemonError::RateLimited(
            "Inspect rate limit exceeded, please slow down".into(),
        ));
    }

    let config = state.config.read().await;
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7".parse().unwrap());
            headers.insert(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse().unwrap());
            headers
        })
        .timeout(std::time::Duration::from_secs(15));

    if let Some(ref proxy_url) = config.proxy.url {
        if !proxy_url.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
    }

    let client = builder
        .build()
        .map_err(|e| DaemonError::Internal(e.to_string()))?;

    let mut builder = client.head(&body.url);
    for (k, v) in &body.headers {
        if k.eq_ignore_ascii_case("range") {
            continue;
        }
        builder = builder.header(k.as_str(), v.as_str());
    }

    if let Some(req) = builder.try_clone().and_then(|b| b.build().ok()) {
        tracing::debug!("HEAD Request to URL: {}", req.url());
    }

    let resp = match builder.send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::debug!(
                "HEAD request returned non-success status: {}. Retrying with GET fallback...",
                r.status()
            );
            let mut get_builder = client.get(&body.url);
            for (k, v) in &body.headers {
                if k.eq_ignore_ascii_case("range") {
                    continue;
                }
                get_builder = get_builder.header(k.as_str(), v.as_str());
            }
            if let Some(req) = get_builder.try_clone().and_then(|b| b.build().ok()) {
                tracing::debug!("GET Fallback Request to URL: {}", req.url());
            }
            let get_resp = get_builder.send().await.map_err(|get_err| {
                DaemonError::BadRequest(format!(
                    "HEAD status {} and GET fallback error: {}",
                    r.status(),
                    get_err
                ))
            })?;
            if !get_resp.status().is_success()
                && get_resp.status() != reqwest::StatusCode::PARTIAL_CONTENT
            {
                return Err(DaemonError::BadRequest(format!(
                    "HEAD status {} and GET fallback status {}",
                    r.status(),
                    get_resp.status()
                )));
            }
            get_resp
        }
        Err(e) => {
            tracing::debug!("HEAD request failed: {}. Retrying with GET fallback...", e);
            let mut get_builder = client.get(&body.url);
            for (k, v) in &body.headers {
                if k.eq_ignore_ascii_case("range") {
                    continue;
                }
                get_builder = get_builder.header(k.as_str(), v.as_str());
            }
            if let Some(req) = get_builder.try_clone().and_then(|b| b.build().ok()) {
                tracing::debug!("GET Fallback Request to URL: {}", req.url());
            }
            let get_resp = get_builder.send().await.map_err(|get_err| {
                DaemonError::BadRequest(format!(
                    "HEAD request failed ({}) and GET fallback failed ({})",
                    e, get_err
                ))
            })?;
            if !get_resp.status().is_success()
                && get_resp.status() != reqwest::StatusCode::PARTIAL_CONTENT
            {
                return Err(DaemonError::BadRequest(format!(
                    "HEAD failed ({}) and GET fallback status {}",
                    e,
                    get_resp.status()
                )));
            }
            get_resp
        }
    };

    let headers = resp.headers();
    let content_length: Option<u64> = headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            // Check Content-Range in case it was a partial content response
            headers
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split('/').next_back())
                .and_then(|s| s.trim().parse().ok())
        });

    let accepts_ranges = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
        || headers
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);

    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let mut filename = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(|cd| {
            let filename_star = cd
                .split(';')
                .find_map(|p| p.trim().strip_prefix("filename*="))
                .and_then(|f| {
                    let f = f.trim_matches('"');
                    f.find("''").map(|idx| {
                        percent_encoding::percent_decode_str(&f[idx + 2..])
                            .decode_utf8_lossy()
                            .into_owned()
                    })
                });
            if filename_star.is_some() {
                return filename_star;
            }
            cd.split(';')
                .find_map(|p| p.trim().strip_prefix("filename="))
                .map(|f| f.trim_matches('"').to_string())
        })
        .or_else(|| {
            body.url
                .split('?')
                .next()
                .and_then(|u| u.split('/').next_back())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    percent_encoding::percent_decode_str(s)
                        .decode_utf8_lossy()
                        .into_owned()
                })
        });

    if let Some(ref mut fname) = filename {
        if !fname.contains('.') {
            if let Some(ct) = &content_type {
                let ext = match ct.split(';').next().unwrap_or("").trim() {
                    "image/jpeg" => "jpg",
                    "image/png" => "png",
                    "image/gif" => "gif",
                    "image/webp" => "webp",
                    "image/svg+xml" => "svg",
                    "video/mp4" => "mp4",
                    "video/webm" => "webm",
                    "video/x-matroska" => "mkv",
                    "audio/mpeg" => "mp3",
                    "audio/wav" => "wav",
                    "audio/ogg" => "ogg",
                    "application/pdf" => "pdf",
                    "application/zip" => "zip",
                    "application/x-rar-compressed" | "application/vnd.rar" => "rar",
                    "application/x-7z-compressed" => "7z",
                    "application/json" => "json",
                    "text/html" => "html",
                    "text/plain" => "txt",
                    "text/csv" => "csv",
                    "application/x-msdownload" | "application/x-dosexec" => "exe",
                    "application/vnd.android.package-archive" => "apk",
                    "application/x-apple-diskimage" => "dmg",
                    _ => "",
                };
                if !ext.is_empty() {
                    *fname = format!("{}.{}", fname, ext);
                }
            }
        }
    }

    Ok(Json(InspectResponse {
        effective_url: resp.url().to_string(),
        filename,
        content_type,
        total_bytes: content_length,
        accepts_ranges,
        ytdlp_supported: false, // Phase 6
    }))
}

// === Intercept (from extension) ===
#[utoipa::path(
    post,
    path = "/api/v1/intercept",
    request_body = AddDownloadRequest,
    responses(
        (status = 200, description = "URL intercepted successfully")
    )
)]
pub async fn intercept_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<vajra_protocol::AddDownloadRequest>,
) -> impl IntoResponse {
    let url = body.url.clone();
    let filename = body
        .filename
        .as_deref()
        .map(vajra_protocol::sanitize_filename)
        .unwrap_or_else(|| "download".to_string());

    // Broadcast the Intercepted event to any connected UI
    state
        .sse
        .send(vajra_protocol::DaemonEvent::Intercepted { url, filename });

    Json(serde_json::json!({ "ok": true }))
}

// ──────────────────────────────────────────────────────────────────────────────
// GET /api/v1/stats
// ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/stats", responses((status = 200, description = "Stats")))]
pub async fn stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let all = state.manager.all_progress().await;
    let active_count = all
        .iter()
        .filter(|p| state_str(&p.state) == "downloading")
        .count();
    let queued_count = all
        .iter()
        .filter(|p| state_str(&p.state) == "queued")
        .count();
    let paused_count = all
        .iter()
        .filter(|p| state_str(&p.state) == "paused")
        .count();
    let aggregate_speed: u64 = all.iter().map(|p| p.speed_bps).sum();

    Json(StatsResponse {
        active_count,
        queued_count,
        paused_count,
        complete_today: 0, // TODO: query DB
        failed_today: 0,   // TODO: query DB
        aggregate_speed_bps: aggregate_speed,
        aggregate_limit_bps: None,
        total_downloaded_bytes: 0,
        daemon_uptime_seconds: state.started_at.elapsed().as_secs(),
        speed_history: state.speed_tracker.get_history().await,
    })
}

// ─── GET/PATCH /api/v1/config ─────────────────────────────────────────────────

fn sanitize_config_for_export(
    mut config: vajra_protocol::DaemonConfig,
    db: &vajra_engine::db::Database,
) -> vajra_protocol::DaemonConfig {
    if db
        .get_credential_by_domain("2captcha.com")
        .ok()
        .flatten()
        .is_some()
    {
        config.captcha_api_key = Some("********".to_string());
    } else {
        config.captcha_api_key = None;
    }
    if config.api_token.is_some() {
        config.api_token = Some("********".to_string());
    }
    if config.s3.secret_key.is_some() {
        config.s3.secret_key = Some("********".to_string());
    }
    if let Some(ref proxy_url) = config.proxy.url {
        if let Ok(mut parsed) = url::Url::parse(proxy_url) {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("");
                let _ = parsed.set_password(None);
                config.proxy.url = Some(parsed.to_string());
            }
        }
    }
    config
}

#[utoipa::path(
    get,
    path = "/api/v1/config",
    responses(
        (status = 200, description = "Daemon configuration", body = DaemonConfig)
    )
)]
pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    let db = state.database.lock().await;
    let sanitized = sanitize_config_for_export(config, &db);
    Json(sanitized)
}

#[utoipa::path(
    patch,
    path = "/api/v1/config",
    request_body = DaemonConfig,
    responses(
        (status = 200, description = "Configuration updated"),
        (status = 400, description = "Invalid configuration", body = ApiErrorResponse)
    )
)]
pub async fn patch_config(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<vajra_protocol::DaemonConfig>,
) -> Result<impl IntoResponse> {
    // 1. Path safety
    let validated_dir = match vajra_protocol::path_security::validate_output_dir(
        std::path::Path::new(&body.default_output_dir),
    ) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            return Err(DaemonError::BadRequest(format!(
                "Invalid default_output_dir: {e}"
            )));
        }
    };
    body.default_output_dir = validated_dir;

    let current_cfg = state.config.read().await.clone();

    // 1b. WebDAV safety: prevent root pivoting and enforce read-only
    if body.webdav_enabled {
        body.webdav_read_only = true;
        if current_cfg.webdav_enabled && body.default_output_dir != current_cfg.default_output_dir {
            return Err(DaemonError::BadRequest(
                "Cannot change default_output_dir while WebDAV is enabled; disable WebDAV first to prevent root pivoting".into(),
            ));
        }
    }

    // 2. Auth token handling: never allow clearing/disabling token or setting placeholder
    let current_token = state.config.read().await.api_token.clone();
    let effective_token = match &body.api_token {
        Some(token) if token != "********" && !token.trim().is_empty() => {
            if !crate::api::auth::is_valid_token(token) {
                return Err(DaemonError::BadRequest(
                    "api_token must be at least 16 characters and cannot be a placeholder".into(),
                ));
            }
            let token_path = vajra_protocol::token_path();
            let _ = crate::api::auth::write_restricted_token_file(&token_path, token.trim());
            Some(token.clone())
        }
        _ => current_token, // Preserve active token
    };
    body.api_token = effective_token;

    // 3. Process captcha key in vault
    if let Some(key) = body.captcha_api_key.clone() {
        let db = state.database.lock().await;
        if key.is_empty() {
            // Delete key
            if let Ok(Some(cred)) = db.get_credential_by_domain("2captcha.com") {
                db.delete_credential(&cred.id)?;
            }
        } else if key != "********" {
            // Upsert: delete existing first, then add
            if let Ok(Some(cred)) = db.get_credential_by_domain("2captcha.com") {
                db.delete_credential(&cred.id)?;
            }
            let cred = vajra_engine::db::VaultCredential {
                id: Uuid::new_v4().to_string(),
                domain: "2captcha.com".to_string(),
                username: "apikey".to_string(),
                password: key,
                created_at: chrono::Utc::now(),
            };
            db.add_credential(&cred)?;
        }
    }

    // Clear key in memory/disk config so it is never saved in plaintext
    body.captcha_api_key = None;

    // 4. Script validation
    if let Some(ref script) = body.post_process_script {
        if !script.is_empty() && !std::path::Path::new(script).is_file() {
            return Err(DaemonError::BadRequest(
                "post_process_script path does not exist".into(),
            ));
        }
    }

    // 5. Numerical bounds
    body.max_concurrent_downloads = body.max_concurrent_downloads.clamp(1, 32);
    body.default_max_connections = body.default_max_connections.clamp(1, 32);

    *state.config.write().await = body.clone();

    // Update global speed limit in manager
    state
        .manager
        .set_global_limit(body.global_speed_limit_bps.unwrap_or(0))
        .await;

    // Update queue settings
    let q_settings = vajra_engine::queue::QueueSettings {
        max_concurrent: body.max_concurrent_downloads as usize,
        scheduler_enabled: body.scheduler_enabled,
        scheduler_start_time: body.scheduler_start_time.clone(),
        scheduler_stop_time: body.scheduler_stop_time.clone(),
        fap_enabled: body.fap_enabled,
        fap_quota_bytes: body.fap_quota_mb * 1024 * 1024,
        fap_time_window_secs: body.fap_window_hours * 3600,
    };
    state.manager.set_settings(q_settings).await;

    // Persist to disk (do not unnecessarily persist generated token into config.json)
    let mut disk_cfg = body.clone();
    let config_path = vajra_protocol::config_path();
    let config_file_had_token = if let Ok(existing_raw) = std::fs::read_to_string(&config_path) {
        if let Ok(existing_json) = serde_json::from_str::<serde_json::Value>(&existing_raw) {
            existing_json
                .get("api_token")
                .and_then(|t| t.as_str())
                .is_some()
        } else {
            false
        }
    } else {
        false
    };
    if !config_file_had_token {
        disk_cfg.api_token = None;
    }
    if let Ok(json) = serde_json::to_string_pretty(&disk_cfg) {
        let _ = std::fs::write(&config_path, json);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── GET /setup ───────────────────────────────────────────────────────────────

const SETUP_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Vajra — Browser Setup</title>
    <style>
        :root { --bg: #0F1117; --accent: #D29922; --text: #E5E7EB; --dim: #9CA3AF; --surface: #1C1F26; }
        body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); display: flex; justify-content: center; min-height: 100vh; }
        .container { max-width: 600px; width: 100%; padding: 40px 20px; box-sizing: border-box; }
        .logo { font-size: 24px; font-weight: bold; letter-spacing: 2px; display: flex; align-items: center; gap: 8px; margin-bottom: 40px; }
        .logo span { color: var(--accent); }
        .card { background: var(--surface); border: 1px solid rgba(255,255,255,0.1); border-radius: 12px; padding: 24px; margin-bottom: 24px; }
        h2 { margin: 0 0 16px 0; font-size: 18px; font-weight: 600; }
        p { margin: 0 0 16px 0; color: var(--dim); line-height: 1.5; font-size: 14px; }
        .status { display: inline-flex; align-items: center; gap: 8px; padding: 6px 12px; border-radius: 20px; font-size: 13px; font-weight: 500; background: rgba(255,255,255,0.05); }
        .status.ok { color: #3FB950; background: rgba(63, 185, 80, 0.1); }
        .status.err { color: #F85149; background: rgba(248, 81, 73, 0.1); }
        .btn { display: inline-block; background: var(--accent); color: #000; text-decoration: none; padding: 12px 24px; border-radius: 6px; font-weight: 600; font-size: 14px; text-align: center; transition: opacity 0.2s; }
        .btn:hover { opacity: 0.9; }
        .steps { display: flex; flex-direction: column; gap: 16px; margin: 24px 0; }
        .step { display: flex; gap: 16px; }
        .step-num { width: 24px; height: 24px; border-radius: 50%; background: rgba(210, 153, 34, 0.1); color: var(--accent); display: flex; align-items: center; justify-content: center; font-weight: bold; font-size: 12px; flex-shrink: 0; }
        .step-text { font-size: 14px; color: var(--text); line-height: 1.5; }
        .step-text code { background: rgba(255,255,255,0.1); padding: 2px 6px; border-radius: 4px; font-family: monospace; font-size: 12px; color: var(--accent); }
    </style>
</head>
<body>
    <div class="container">
        <div class="logo">⚡ VAJ<span>RA</span></div>
        
        <div class="card">
            <h2>Daemon Status</h2>
            <div id="status" class="status">Checking...</div>
        </div>

        <div class="card">
            <h2>1. Install Chrome Extension</h2>
            <p>Vajra intercepts downloads directly from your browser. Follow these steps to install the extension manually (developer mode) until it's published on the Chrome Web Store.</p>
            
            <div class="steps">
                <div class="step">
                    <div class="step-num">1</div>
                    <div class="step-text">Open <code>chrome://extensions</code> in a new tab.</div>
                </div>
                <div class="step">
                    <div class="step-num">2</div>
                    <div class="step-text">Enable <strong>Developer mode</strong> (toggle in the top right corner).</div>
                </div>
                <div class="step">
                    <div class="step-num">3</div>
                    <div class="step-text">Click <strong>Load unpacked</strong> and select the <code>browser-extension</code> folder inside the Vajra project directory.</div>
                </div>
            </div>

            <a href="https://chromewebstore.google.com" class="btn" target="_blank" style="opacity: 0.5; cursor: not-allowed; pointer-events: none;">Chrome Web Store (Coming Soon)</a>
        </div>

        <div class="card">
            <h2>Already installed?</h2>
            <p>If the extension is active, it will automatically connect to Vajra.</p>
            <div id="ext-status" class="status" style="margin-top: 8px;">Checking...</div>
        </div>
    </div>

    <script>
        async function checkDaemon() {
            const el = document.getElementById('status');
            try {
                const r = await fetch('/health');
                if (r.ok) {
                    el.className = 'status ok';
                    el.innerHTML = 'Daemon Connected ✓';
                } else throw new Error();
            } catch {
                el.className = 'status err';
                el.innerHTML = 'Daemon not running';
            }
        }

        async function checkExtension() {
            const el = document.getElementById('ext-status');
            setInterval(() => {
                if (document.documentElement.getAttribute('data-vajra-ext')) {
                    el.className = 'status ok';
                    el.innerHTML = 'Extension Active ✓';
                }
            }, 500);
        }

        checkDaemon();
        checkExtension();
        setInterval(checkDaemon, 5000);
    </script>
</body>
</html>"#;

pub async fn browser_setup() -> impl IntoResponse {
    Html(SETUP_HTML)
}

// ─── Vault Handlers ───────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/vault",
    responses(
        (status = 200, description = "List vault credentials", body = [VaultCredentialResponse])
    )
)]
pub async fn get_vault_credentials(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    let creds = db.get_credentials()?;

    let response: Vec<vajra_protocol::VaultCredentialResponse> = creds
        .into_iter()
        .map(|c| vajra_protocol::VaultCredentialResponse {
            id: c.id,
            domain: c.domain,
            username: c.username,
            created_at: c.created_at.timestamp(),
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/vault",
    request_body = AddVaultCredentialRequest,
    responses(
        (status = 201, description = "Credential added", body = VaultCredentialResponse),
        (status = 400, description = "Bad request", body = ApiErrorResponse)
    )
)]
pub async fn add_vault_credential(
    State(state): State<Arc<AppState>>,
    Json(body): Json<vajra_protocol::AddVaultCredentialRequest>,
) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    let id = Uuid::new_v4().to_string();
    let cred = vajra_engine::db::VaultCredential {
        id: id.clone(),
        domain: body.domain.clone(),
        username: body.username.clone(),
        password: body.password.clone(),
        created_at: chrono::Utc::now(),
    };
    db.add_credential(&cred)?;

    Ok((
        StatusCode::CREATED,
        Json(vajra_protocol::VaultCredentialResponse {
            id,
            domain: cred.domain,
            username: cred.username,
            created_at: cred.created_at.timestamp(),
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/vault/{id}",
    responses(
        (status = 204, description = "Credential deleted")
    )
)]
pub async fn delete_vault_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    db.delete_credential(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── RSS Endpoints ────────────────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/rss", request_body = AddRssFeedRequest, responses((status = 201, description = "RSS Feed Added")))]
pub async fn add_rss_feed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<vajra_protocol::AddRssFeedRequest>,
) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    let id = Uuid::new_v4().to_string();

    // Optional: Fetch the feed immediately to get the title
    let mut title = String::new();
    if let Ok(response) = reqwest::get(&body.url).await {
        if let Ok(bytes) = response.bytes().await {
            if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                title = channel.title().to_string();
            }
        }
    }
    if title.is_empty() {
        title = body.url.clone();
    }

    db.add_rss_feed(&id, &body.url, &title)?;

    Ok((
        StatusCode::CREATED,
        Json(vajra_protocol::RssFeed {
            id,
            url: body.url,
            title,
            created_at: chrono::Utc::now().timestamp(),
        }),
    ))
}

#[utoipa::path(get, path = "/api/v1/rss", responses((status = 200, description = "List RSS Feeds", body = [RssFeed])))]
pub async fn get_all_rss_feeds(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    let feeds = db.get_all_rss_feeds()?;
    Ok(Json(feeds))
}

#[utoipa::path(delete, path = "/api/v1/rss/{id}", responses((status = 204, description = "RSS Feed Deleted")))]
pub async fn delete_rss_feed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let db = state.database.lock().await;
    db.delete_rss_feed(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Collaboration (Phase 5) ──────────────────────────────────────────────────

use vajra_engine::db::AuditLog;

pub async fn get_audit_logs(State(state): State<Arc<AppState>>) -> Result<Json<Vec<AuditLog>>> {
    let db = state.database.lock().await;
    let logs = db.get_audit_logs(100)?;
    Ok(Json(logs))
}

pub async fn get_shared_queue(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Returns a simplified view of the active queue for sharing
    let statuses = state.manager.all_progress().await;
    let simplified: Vec<serde_json::Value> = statuses
        .into_iter()
        .map(|progress| {
            serde_json::json!({
                "id": progress.id,
                "filename": progress.filename,
                "bytes_downloaded": progress.bytes_downloaded,
                "total_bytes": progress.total_bytes,
                "progress_fraction": progress.progress_fraction,
                "state": progress.state,
            })
        })
        .collect();

    Json(simplified)
}

// ─── GET /api/v1/config/export ────────────────────────────────────────────────
#[utoipa::path(
    get,
    path = "/api/v1/config/export",
    responses(
        (status = 200, description = "Exported daemon configuration", body = DaemonConfig)
    )
)]
pub async fn export_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    let db = state.database.lock().await;
    let sanitized = sanitize_config_for_export(config, &db);
    Json(sanitized)
}

// ─── POST /api/v1/config/import ───────────────────────────────────────────────
#[utoipa::path(
    post,
    path = "/api/v1/config/import",
    request_body = DaemonConfig,
    responses(
        (status = 200, description = "Configuration imported"),
        (status = 400, description = "Invalid configuration", body = ApiErrorResponse)
    )
)]
pub async fn import_config(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<vajra_protocol::DaemonConfig>,
) -> Result<impl IntoResponse> {
    // 1. Path safety check
    let validated_dir = match vajra_protocol::path_security::validate_output_dir(
        std::path::Path::new(&body.default_output_dir),
    ) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            return Err(DaemonError::BadRequest(format!(
                "Invalid default_output_dir in imported config: {e}"
            )));
        }
    };
    body.default_output_dir = validated_dir;

    let current_cfg = state.config.read().await.clone();

    // 1b. WebDAV safety: prevent root pivoting and enforce read-only
    if body.webdav_enabled {
        body.webdav_read_only = true;
        if current_cfg.webdav_enabled && body.default_output_dir != current_cfg.default_output_dir {
            return Err(DaemonError::BadRequest(
                "Cannot change default_output_dir while WebDAV is enabled; disable WebDAV first to prevent root pivoting".into(),
            ));
        }
    }

    // 2. Auth safety: do NOT allow import to disable API authentication or set placeholder
    let current_token = state.config.read().await.api_token.clone();
    let effective_token = match &body.api_token {
        Some(token) if token != "********" && !token.trim().is_empty() => {
            if !crate::api::auth::is_valid_token(token) {
                return Err(DaemonError::BadRequest(
                    "api_token must be at least 16 characters and cannot be a placeholder".into(),
                ));
            }
            let token_path = vajra_protocol::token_path();
            let _ = crate::api::auth::write_restricted_token_file(&token_path, token.trim());
            Some(token.clone())
        }
        _ => current_token, // Preserve active token
    };
    body.api_token = effective_token;

    // 4. Script validation
    if let Some(ref script) = body.post_process_script {
        if !script.is_empty() && !std::path::Path::new(script).is_file() {
            return Err(DaemonError::BadRequest(
                "Imported post_process_script does not exist".into(),
            ));
        }
    }

    // 5. Numerical bounds
    body.max_concurrent_downloads = body.max_concurrent_downloads.clamp(1, 32);
    body.default_max_connections = body.default_max_connections.clamp(1, 32);

    // Update in-memory
    *state.config.write().await = body.clone();

    // Persist to DB settings
    let db = state.database.lock().await;
    let s = vajra_engine::db::AppSettings {
        default_download_dir: body.default_output_dir.clone(),
        max_concurrent_downloads: body.max_concurrent_downloads as u32,
        global_speed_limit_bps: body.global_speed_limit_bps.unwrap_or(0),
        start_minimized: false,
        minimize_to_tray: true,
        sound_on_complete: body.sound_on_complete,
        dark_mode: true,
        browser_integration: true,
        auto_start_downloads: true,
        default_connections_per_download: body.default_max_connections as u32,
        scheduler_enabled: body.scheduler_enabled,
        scheduler_start_time: body.scheduler_start_time.clone(),
        scheduler_stop_time: body.scheduler_stop_time.clone(),
        client_id: db.load_settings().map(|x| x.client_id).unwrap_or_default(),
    };
    db.save_settings(&s)
        .map_err(|e| DaemonError::Internal(e.to_string()))?;

    // Update manager queue settings
    let q_settings = vajra_engine::queue::QueueSettings {
        max_concurrent: body.max_concurrent_downloads as usize,
        scheduler_enabled: body.scheduler_enabled,
        scheduler_start_time: body.scheduler_start_time.clone(),
        scheduler_stop_time: body.scheduler_stop_time.clone(),
        fap_enabled: body.fap_enabled,
        fap_quota_bytes: body.fap_quota_mb * 1024 * 1024,
        fap_time_window_secs: body.fap_window_hours * 3600,
    };
    state.manager.set_settings(q_settings).await;

    // Persist to disk (safe without exposing generated token in config.json)
    let mut disk_cfg = body.clone();
    let config_path = vajra_protocol::config_path();
    let config_file_had_token = if let Ok(existing_raw) = std::fs::read_to_string(&config_path) {
        if let Ok(existing_json) = serde_json::from_str::<serde_json::Value>(&existing_raw) {
            existing_json
                .get("api_token")
                .and_then(|t| t.as_str())
                .is_some()
        } else {
            false
        }
    } else {
        false
    };
    if !config_file_had_token {
        disk_cfg.api_token = None;
    }
    if let Ok(json) = serde_json::to_string_pretty(&disk_cfg) {
        let _ = std::fs::write(&config_path, json);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
// ─── POST /api/v1/downloads/:id/preview ───────────────────────────────────────
#[utoipa::path(
    post,
    path = "/api/v1/downloads/{id}/preview",
    responses(
        (status = 200, description = "File preview launched or retrieved"),
        (status = 400, description = "Unsafe or unsupported preview target", body = ApiErrorResponse),
        (status = 404, description = "Download not found", body = ApiErrorResponse)
    )
)]
pub async fn preview_download(
    Path(id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse> {
    use std::io::{Read, Write};

    let progress = state
        .manager
        .progress(id)
        .await
        .ok_or_else(|| DaemonError::NotFound(id))?;

    let src_path = if !progress.dest_path.is_empty() {
        let p = std::path::PathBuf::from(&progress.dest_path);
        if p.is_dir() {
            p.join(&progress.filename)
        } else {
            p
        }
    } else if let Some(req) = state.manager.get_request(id).await {
        let name = req.filename.as_deref().unwrap_or(&progress.filename);
        req.dest_dir.join(name)
    } else {
        std::path::PathBuf::new()
    };

    if !src_path.exists() {
        return Err(DaemonError::BadRequest(
            "Download file does not exist yet".to_string(),
        ));
    }

    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Explicitly reject executable / script / shortcut formats
    const REJECTED_EXTENSIONS: &[&str] = &[
        "exe", "bat", "cmd", "vbs", "vbe", "js", "jse", "wsf", "wsh", "ps1", "ps1xml", "ps2",
        "ps2xml", "psc1", "psc2", "msh", "msh1", "msh2", "mshxml", "msh1xml", "msh2xml", "msi",
        "msp", "mst", "com", "scr", "hta", "cpl", "jar", "reg", "pif", "lnk", "url", "dll", "sys",
        "drv", "bin", "iso", "img", "sh", "bash", "psm1", "psd1",
    ];

    if REJECTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(DaemonError::BadRequest(format!(
            "Preview is strictly prohibited for executable/script format: .{ext}"
        )));
    }

    // Safe media / preview allowlist
    const SAFE_PREVIEW_EXTENSIONS: &[&str] = &[
        // Images
        "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico", "tiff", "avif",
        // Audio
        "mp3", "wav", "ogg", "flac", "m4a", "aac", "wma", "opus", // Video
        "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", // Text / Documents
        "pdf", "txt", "md", "csv", "json", "xml", "log",
    ];

    if !SAFE_PREVIEW_EXTENSIONS.contains(&ext.as_str()) {
        return Err(DaemonError::BadRequest(format!(
            "File type '.{ext}' is not supported for safe preview"
        )));
    }

    // Determine target preview path in temporary directory with unique UUID
    let filename = src_path
        .file_name()
        .ok_or_else(|| DaemonError::Internal("Invalid filename".to_string()))?;

    let temp_dir = std::env::temp_dir();
    let preview_path = temp_dir.join(format!(
        "preview_{}_{}",
        Uuid::new_v4(),
        filename.to_string_lossy()
    ));

    // Copy the partial file (up to the current downloaded bytes size to avoid copy bloat of huge unallocated files)
    let mut src_file =
        std::fs::File::open(src_path).map_err(|e| DaemonError::Internal(e.to_string()))?;
    let mut dst_file =
        std::fs::File::create(&preview_path).map_err(|e| DaemonError::Internal(e.to_string()))?;

    let copy_limit = progress.bytes_downloaded.clamp(1024, 10 * 1024 * 1024);
    let mut buffer = vec![0u8; 8192];
    let mut total_copied = 0;

    while total_copied < copy_limit {
        let to_read = (copy_limit - total_copied).min(buffer.len() as u64) as usize;
        let read = src_file
            .read(&mut buffer[..to_read])
            .map_err(|e| DaemonError::Internal(e.to_string()))?;
        if read == 0 {
            break;
        }
        dst_file
            .write_all(&buffer[..read])
            .map_err(|e| DaemonError::Internal(e.to_string()))?;
        total_copied += read as u64;
    }

    // Open the preview file with the default system application (non-blocking, direct process execution without shell)
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(&preview_path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(&preview_path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(&preview_path)
            .spawn();
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "preview_path": preview_path.to_string_lossy()
    })))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use tokio::sync::{Mutex, RwLock};
    use vajra_engine::download_task::{DownloadRequest, DownloadTask, TaskState};
    use vajra_protocol::{BulkAction, BulkActionRequest, Priority, QueueType};

    use super::*;

    async fn create_test_state() -> (Arc<AppState>, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_vajra.db");
        let database = vajra_engine::db::Database::open(&db_path).unwrap();
        let manager = vajra_engine::DownloadManager::new(
            vajra_engine::queue::QueueSettings {
                max_concurrent: 4,
                ..Default::default()
            },
            0,
        );
        let sse = crate::api::sse::SseBroadcaster::new();
        let speed_tracker = crate::speed_history::SpeedTracker::new(60);
        let config = vajra_protocol::DaemonConfig::default();
        let ab_mgr = vajra_engine::ab_test::ExperimentManager::new("test-client".to_string());
        let ab_test = Arc::new(ab_mgr);
        let state = Arc::new(AppState {
            manager,
            database: Mutex::new(database),
            config: RwLock::new(config),
            sse,
            speed_tracker: speed_tracker.clone(),
            started_at: std::time::Instant::now(),
            shutdown_tx: Mutex::new(None),
            ab_test,
            inspect_limiter: Mutex::new(Vec::new()),
            spider_limiter: Mutex::new(Vec::new()),
            add_download_limiter: Mutex::new(Vec::new()),
        });
        (state, temp_dir)
    }

    fn sample_req(dest_dir: std::path::PathBuf, filename: &str) -> DownloadRequest {
        DownloadRequest {
            url: format!("http://example.com/{}", filename),
            mirrors: vec![],
            dest_dir,
            filename: Some(filename.to_string()),
            timeout_secs: None,
            connect_timeout_secs: None,
            max_connections: 1,
            speed_limit: 0,
            throttle: None,
            delete_on_failure: false,
            queue_type: QueueType::Standard,
            sync_interval_secs: 3600,
            referrer: None,
            cookie_header: None,
            user_agent: None,
            authorization: None,
            proxy: None,
            proxies: vec![],
            local_address: None,
            use_ytdlp: false,
            ytdlp_format: None,
            ytdlp_subtitles: false,
            ytdlp_playlist: false,
            use_http3: false,
            expected_hash: None,
            auto_extract: false,
            post_processing_script: None,
            av_scan_path: None,
            av_scan_args: vec![],
            schedule_at: None,
            daemon_config: None,
            priority: Priority::Normal,
            tags: vec![],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_bulk_action_validation_rejections() {
        let (state, _temp) = create_test_state().await;

        // 1. >500 IDs submitted before deduplication MUST be rejected with 400
        let ids_501: Vec<Uuid> = (0..501).map(|_| Uuid::new_v4()).collect();
        let res = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: ids_501,
                action: BulkAction::Pause,
                all: false,
                delete_file: false,
            }),
        )
        .await;
        assert!(matches!(res, Err(DaemonError::BadRequest(_))));

        // 2. all: true combined with IDs MUST be rejected with 400
        let res_all_and_ids = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: vec![Uuid::new_v4()],
                action: BulkAction::Pause,
                all: true,
                delete_file: false,
            }),
        )
        .await;
        assert!(matches!(res_all_and_ids, Err(DaemonError::BadRequest(_))));

        // 3. all: false combined with empty IDs MUST be rejected with 400
        let res_empty = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: vec![],
                action: BulkAction::Pause,
                all: false,
                delete_file: false,
            }),
        )
        .await;
        assert!(matches!(res_empty, Err(DaemonError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_bulk_action_deduplication() {
        let (state, _temp) = create_test_state().await;
        let dup_id = Uuid::new_v4();

        // 3 identical IDs in request
        let res = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: vec![dup_id, dup_id, dup_id],
                action: BulkAction::Pause,
                all: false,
                delete_file: false,
            }),
        )
        .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_clear_completed_removes_active_and_job_preserves_history() {
        let (state, temp) = create_test_state().await;
        let id = Uuid::new_v4();
        let req = sample_req(temp.path().to_path_buf(), "done.bin");

        // 1. Add restored task in Completed state
        let task = DownloadTask::new_restored(
            id,
            req.clone(),
            TaskState::Completed,
            1000,
            1000,
            "done.bin".into(),
            temp.path().join("done.bin").to_string_lossy().into_owned(),
            None,
        );
        state.manager.add_restored(id, req.clone(), task).await;

        // 2. Add DB job and DB history entry
        {
            let db = state.database.lock().await;
            db.upsert_job(&vajra_engine::db::JobRecord {
                id: id.to_string(),
                request_json: serde_json::to_string(&req).unwrap(),
                state: "completed".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();

            db.insert_history(&vajra_engine::db::HistoryEntry {
                id: id.to_string(),
                url: req.url.clone(),
                filename: "done.bin".to_string(),
                dest_path: temp.path().join("done.bin").to_string_lossy().into_owned(),
                total_bytes: 1000,
                speed_avg_bps: 500,
                status: "completed".to_string(),
                completed_at: Utc::now(),
                tags: vec![],
            })
            .unwrap();
        }

        // Verify pre-conditions
        assert!(state.manager.progress(id).await.is_some());
        {
            let db = state.database.lock().await;
            assert!(db.get_job(&id.to_string()).unwrap().is_some());
            assert!(db.get_history_entry(&id.to_string()).unwrap().is_some());
        }

        // 3. Execute clear_completed_internal
        let res = clear_completed_internal(&state, id).await;
        assert!(
            res.is_ok(),
            "clear_completed on completed download must succeed"
        );

        // 4. Verify post-conditions:
        // Absent from active queue
        assert!(state.manager.progress(id).await.is_none());

        // Absent from SQLite jobs table
        {
            let db = state.database.lock().await;
            assert!(
                db.get_job(&id.to_string()).unwrap().is_none(),
                "Job must be removed from active jobs"
            );

            // STILL PRESENT in SQLite history table (Regression test for constraint 6!)
            let hist = db.get_history_entry(&id.to_string()).unwrap();
            assert!(
                hist.is_some(),
                "History entry MUST be preserved after clear_completed"
            );
            assert_eq!(hist.unwrap().id, id.to_string());
        }
    }

    #[tokio::test]
    async fn test_clear_completed_rejects_active_downloads() {
        let (state, temp) = create_test_state().await;
        let id = Uuid::new_v4();
        let req = sample_req(temp.path().to_path_buf(), "active.bin");

        // Restored in Paused state (not Completed)
        let task = DownloadTask::new_restored(
            id,
            req.clone(),
            TaskState::Paused,
            100,
            1000,
            "active.bin".into(),
            temp.path()
                .join("active.bin")
                .to_string_lossy()
                .into_owned(),
            None,
        );
        state.manager.add_restored(id, req.clone(), task).await;

        let res = clear_completed_internal(&state, id).await;
        assert!(res.is_err());
        let (code, _) = res.unwrap_err();
        assert_eq!(code, "invalid_state");

        // Remains in active queue
        assert!(state.manager.progress(id).await.is_some());
    }

    #[tokio::test]
    async fn test_bulk_delete_with_simulated_filesystem_failure() {
        let (state, temp) = create_test_state().await;
        let id = Uuid::new_v4();
        let file_path = temp.path().join("locked_file.bin");
        std::fs::write(&file_path, b"test payload for delete").unwrap();

        let req = sample_req(temp.path().to_path_buf(), "locked_file.bin");

        // Add task and DB records
        let task = DownloadTask::new_restored(
            id,
            req.clone(),
            TaskState::Completed,
            23,
            23,
            "locked_file.bin".into(),
            file_path.to_string_lossy().into_owned(),
            None,
        );
        state.manager.add_restored(id, req.clone(), task).await;

        {
            let db = state.database.lock().await;
            db.upsert_job(&vajra_engine::db::JobRecord {
                id: id.to_string(),
                request_json: serde_json::to_string(&req).unwrap(),
                state: "completed".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
            db.insert_history(&vajra_engine::db::HistoryEntry {
                id: id.to_string(),
                url: req.url.clone(),
                filename: "locked_file.bin".to_string(),
                dest_path: file_path.to_string_lossy().into_owned(),
                total_bytes: 23,
                speed_avg_bps: 100,
                status: "completed".to_string(),
                completed_at: Utc::now(),
                tags: vec![],
            })
            .unwrap();
        }

        // Simulate filesystem failure by exclusively locking the file (share_mode = 0 prevents deletion on Windows)
        #[cfg(target_os = "windows")]
        use std::os::windows::fs::OpenOptionsExt;

        let mut opts = std::fs::OpenOptions::new();
        opts.read(true).write(true);
        #[cfg(target_os = "windows")]
        opts.share_mode(0);
        let _lock = opts.open(&file_path);

        #[cfg(target_os = "windows")]
        if _lock.is_ok() {
            // Attempt deletion with delete_file: true
            let res = delete_download_internal(&state, id, true).await;
            assert!(
                matches!(res, Err(DaemonError::Io(_))),
                "Locked file deletion must return DaemonError::Io"
            );

            // DB records MUST NOT be deleted when I/O error occurred!
            let db = state.database.lock().await;
            assert!(
                db.get_job(&id.to_string()).unwrap().is_some(),
                "Job record must NOT be deleted on filesystem error"
            );
            assert!(
                db.get_history_entry(&id.to_string()).unwrap().is_some(),
                "History entry must NOT be deleted on filesystem error"
            );
        }

        // Drop the lock
        drop(_lock);

        // Now deletion should succeed
        let ok_res = delete_download_internal(&state, id, true).await;
        assert!(ok_res.is_ok(), "Deletion must succeed once lock is dropped");
        assert!(!file_path.exists(), "Physical file must be deleted");

        // DB records should now be removed
        let db = state.database.lock().await;
        assert!(db.get_job(&id.to_string()).unwrap().is_none());
        assert!(db.get_history_entry(&id.to_string()).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_bulk_handler_emits_no_fake_state_change_events() {
        let (state, temp) = create_test_state().await;
        let mut sse_rx = state.sse.subscribe();

        let id = Uuid::new_v4();
        let req = sample_req(temp.path().to_path_buf(), "test.bin");
        state.manager.add_with_id(id, req).await;

        // Call bulk pause
        let _ = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: vec![id],
                action: BulkAction::Pause,
                all: false,
                delete_file: false,
            }),
        )
        .await;

        // Drain any SSE events currently in queue and ensure NO fake StateChange was dispatched by bulk_action
        let mut manufactured_state_change = false;
        while let Ok(event) = sse_rx.try_recv() {
            if matches!(*event, vajra_protocol::DaemonEvent::StateChange { .. }) {
                manufactured_state_change = true;
            }
        }
        assert!(
            !manufactured_state_change,
            "bulk_action MUST NOT manufacture artificial StateChange SSE events"
        );
    }

    #[tokio::test]
    async fn test_bulk_action_retry_state_matrix() {
        let (state, temp) = create_test_state().await;
        let id_failed = Uuid::new_v4();
        let id_cancelled = Uuid::new_v4();
        let id_completed = Uuid::new_v4();
        let id_active = Uuid::new_v4();

        let req = sample_req(temp.path().to_path_buf(), "test.bin");

        // 1. Add tasks in various states
        state
            .manager
            .add_restored(
                id_failed,
                req.clone(),
                DownloadTask::new_restored(
                    id_failed,
                    req.clone(),
                    TaskState::Failed,
                    0,
                    100,
                    "f.bin".into(),
                    "/tmp/f.bin".into(),
                    Some("err".into()),
                ),
            )
            .await;

        state
            .manager
            .add_restored(
                id_cancelled,
                req.clone(),
                DownloadTask::new_restored(
                    id_cancelled,
                    req.clone(),
                    TaskState::Cancelled,
                    0,
                    100,
                    "c.bin".into(),
                    "/tmp/c.bin".into(),
                    None,
                ),
            )
            .await;

        state
            .manager
            .add_restored(
                id_completed,
                req.clone(),
                DownloadTask::new_restored(
                    id_completed,
                    req.clone(),
                    TaskState::Completed,
                    100,
                    100,
                    "comp.bin".into(),
                    "/tmp/comp.bin".into(),
                    None,
                ),
            )
            .await;

        state.manager.add_with_id(id_active, req.clone()).await;

        // 2. Bulk retry with all 4 IDs
        let res = bulk_action(
            State(Arc::clone(&state)),
            Json(BulkActionRequest {
                ids: vec![id_failed, id_cancelled, id_completed, id_active],
                action: BulkAction::Retry,
                all: false,
                delete_file: false,
            }),
        )
        .await;

        assert!(res.is_ok());
        let body = res.unwrap();
        // Failed and Cancelled MUST succeed
        assert!(body.0.succeeded.contains(&id_failed));
        assert!(body.0.succeeded.contains(&id_cancelled));

        // Completed and Active MUST fail with invalid_state
        let failed_map: std::collections::HashMap<Uuid, String> =
            body.0.failed.into_iter().map(|f| (f.id, f.code)).collect();
        assert_eq!(
            failed_map.get(&id_completed).map(|s| s.as_str()),
            Some("invalid_state")
        );
        assert_eq!(
            failed_map.get(&id_active).map(|s| s.as_str()),
            Some("invalid_state")
        );
    }
}
