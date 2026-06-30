//! REST API handlers — one async fn per endpoint.

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{Path, Query, State, ws::{WebSocketUpgrade, WebSocket, Message}},
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
    AddDownloadRequest, AddDownloadResponse, DownloadAction, DownloadInfo, DownloadList,
    InspectRequest, InspectResponse, PatchDownloadRequest, StatsResponse,
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
    // Validate scheme
    let trimmed_url = body.url.trim();
    let lower_url = trimmed_url.to_lowercase();
    if !lower_url.starts_with("http://")
        && !lower_url.starts_with("https://")
        && !lower_url.starts_with("magnet:")
    {
        return Err(DaemonError::BadRequest(
            "Only HTTP(S) and magnet: URLs are supported".into(),
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
        PathBuf::from(dir) // explicit override from caller
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
            tracing::info!("Consumer Cloud share link detected: '{}'. Attempting auto-translation...", target_url);
            match vajra_engine::cloud::translate_cloud_link(&target_url).await {
                Ok(translated) => {
                    tracing::info!("Successfully translated cloud link to direct download: '{}'", translated);
                    target_url = translated;
                }
                Err(e) => {
                    tracing::warn!("Failed to translate cloud link: {}. Downloading original URL.", e);
                }
            }
        }

        let tcp_multiplexing_opt = state.ab_test.is_enabled("tcp_multiplexing_opt");
        let adaptive_chunk_v2 = state.ab_test.is_enabled("adaptive_chunk_v2");

        let request = DownloadRequest {
            url: target_url,
            mirrors: vec![],
            dest_dir: output_dir,
            filename: body.filename.clone(),
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
        post_processing_script: body
            .post_processing_script
            .clone()
            .or_else(|| config.post_process_script.clone()),
        av_scan_path: config.av_scan_path.clone(),
        av_scan_args: config.av_scan_args.clone(),
        schedule_at: body.schedule_at,
        queue_type: body.queue_type.clone().unwrap_or_default(),
        sync_interval_secs: body.sync_interval_secs.unwrap_or(3600),
        priority: vajra_protocol::Priority::Normal,
        tags: body.tags.clone().unwrap_or_default(),
        daemon_config: Some((*config).clone()),
    };

    drop(config);
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Persist job to DB
    {
        let db = state.database.lock().await;
        db.upsert_job(&vajra_engine::db::JobRecord {
            id: id.to_string(),
            request_json: serde_json::to_string(&request).unwrap_or_default(),
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

    let total = items.len();
    let paged: Vec<DownloadInfo> = items
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .collect();

    Ok(Json(DownloadList {
        total,
        limit: params.limit,
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
        let new_filename = new_filename.trim();
        if new_filename.is_empty() || new_filename.contains('/') || new_filename.contains('\\') {
            return Err(DaemonError::BadRequest(
                "Invalid filename: cannot be empty or contain path separators".into(),
            ));
        }

        if state.manager.update_filename(id, new_filename.to_string()).await.is_err() {
            return Err(DaemonError::NotFound(id));
        }

        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                request.filename = Some(new_filename.to_string());
                if let Ok(new_json) = serde_json::to_string(&request) {
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
        let lower_new_url = new_url.trim().to_lowercase();
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
                if let Ok(new_json) = serde_json::to_string(&request) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        }
    }

    // 2. Handle Settings change (speed limit, max connections)
    if body.speed_limit_bps.is_some() || body.max_connections.is_some() {
        let speed_limit = body.speed_limit_bps.map(|opt| opt.unwrap_or(0));
        let max_connections = body.max_connections;

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
                if let Ok(new_json) = serde_json::to_string(&request) {
                    job.request_json = new_json;
                    job.updated_at = chrono::Utc::now();
                    let _ = db.upsert_job(&job);
                }
            }
        }
    }

    // 2.5 Handle Tags change
    if let Some(new_tags) = &body.tags {
        if state.manager.update_tags(id, new_tags.clone()).await.is_err() {
            // Might not be active anymore, but we can still update the DB
        }
        let db = state.database.lock().await;
        if let Ok(Some(mut job)) = db.get_job(&id.to_string()) {
            if let Ok(mut request) = serde_json::from_str::<
                vajra_engine::download_task::DownloadRequest,
            >(&job.request_json)
            {
                request.tags = new_tags.clone();
                if let Ok(new_json) = serde_json::to_string(&request) {
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
            DownloadAction::Resume | DownloadAction::Retry => state.manager.resume(id).await,
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
            DownloadAction::Resume | DownloadAction::Retry => vajra_protocol::DownloadStatus::Connecting,
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

#[utoipa::path(delete, path = "/api/v1/downloads/{id}", responses((status = 200, description = "Download deleted")))]
pub async fn delete_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteParams>,
) -> Result<impl IntoResponse> {
    let progress = state.manager.progress(id).await;

    // Determine paths to delete before removing from database/manager
    let mut file_path_to_delete = None;
    let mut state_path_to_delete = None;

    if params.delete_file {
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

        if file_path_to_delete.is_none() {
            let db = state.database.lock().await;
            if let Ok(Some(hist)) = db.get_history_entry(&id.to_string()) {
                if !hist.dest_path.is_empty() {
                    let file_path = std::path::PathBuf::from(&hist.dest_path);
                    file_path_to_delete = Some(file_path);
                    let filename = std::path::Path::new(&hist.dest_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if let Some(parent) = std::path::Path::new(&hist.dest_path).parent() {
                        state_path_to_delete =
                            Some(parent.join(format!(".{}.vajra.state", filename)));
                    }
                }
            } else if let Ok(Some(job)) = db.get_job(&id.to_string()) {
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

    state.manager.cancel(id).await;
    {
        let db = state.database.lock().await;
        let _ = db.delete_job(&id.to_string());
        let _ = db.delete_history_entry(&id.to_string());
    }

    if params.delete_file {
        if let Some(path) = file_path_to_delete {
            if path.exists() {
                for _ in 0..30 {
                    let result = if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    if result.is_ok() {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                }
            }
        }
        if let Some(path) = state_path_to_delete {
            if path.exists() {
                for _ in 0..30 {
                    let result = if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    if result.is_ok() {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                }
            }
        }
    }

    state.sse.send(vajra_protocol::DaemonEvent::Removed { id });
    Ok(Json(serde_json::json!({ "id": id, "ok": true })))
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
                if futures_util::SinkExt::send(&mut sender, Message::Text(json.into())).await.is_err() {
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

pub async fn inspect_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InspectRequest>,
) -> Result<impl IntoResponse> {
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
        println!("[DEBUG] HEAD Request to URL: {}", req.url());
        println!("[DEBUG] HEAD Request Headers: {:?}", req.headers());
    }

    let resp = match builder.send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            println!("[DEBUG] HEAD request returned non-success status: {}. Retrying with GET fallback...", r.status());
            let mut get_builder = client.get(&body.url);
            for (k, v) in &body.headers {
                if k.eq_ignore_ascii_case("range") {
                    continue;
                }
                get_builder = get_builder.header(k.as_str(), v.as_str());
            }
            if let Some(req) = get_builder.try_clone().and_then(|b| b.build().ok()) {
                println!("[DEBUG] GET Fallback Request to URL: {}", req.url());
            }
            let get_resp = get_builder.send().await.map_err(|get_err| {
                DaemonError::BadRequest(format!("HEAD status {} and GET fallback error: {}", r.status(), get_err))
            })?;
            if !get_resp.status().is_success() && get_resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(DaemonError::BadRequest(format!("HEAD status {} and GET fallback status {}", r.status(), get_resp.status())));
            }
            get_resp
        }
        Err(e) => {
            println!("[DEBUG] HEAD request failed: {}. Retrying with GET fallback...", e);
            let mut get_builder = client.get(&body.url);
            for (k, v) in &body.headers {
                if k.eq_ignore_ascii_case("range") {
                    continue;
                }
                get_builder = get_builder.header(k.as_str(), v.as_str());
            }
            if let Some(req) = get_builder.try_clone().and_then(|b| b.build().ok()) {
                println!("[DEBUG] GET Fallback Request to URL: {}", req.url());
            }
            let get_resp = get_builder.send().await.map_err(|get_err| {
                DaemonError::BadRequest(format!("HEAD request failed ({}) and GET fallback failed ({})", e, get_err))
            })?;
            if !get_resp.status().is_success() && get_resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(DaemonError::BadRequest(format!("HEAD failed ({}) and GET fallback status {}", e, get_resp.status())));
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
pub async fn intercept_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<vajra_protocol::AddDownloadRequest>,
) -> impl IntoResponse {
    let url = body.url.clone();
    let filename = body
        .filename
        .clone()
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

pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut config = state.config.read().await.clone();
    let db = state.database.lock().await;
    if let Ok(Some(_)) = db.get_credential_by_domain("2captcha.com") {
        config.captcha_api_key = Some("********".to_string());
    } else {
        config.captcha_api_key = None;
    }
    Json(config)
}

pub async fn patch_config(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<vajra_protocol::DaemonConfig>,
) -> Result<impl IntoResponse> {
    // Process captcha key in vault
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

    // Persist to disk (best-effort)
    if let Ok(json) = serde_json::to_string_pretty(&body) {
        let _ = std::fs::write(vajra_protocol::config_path(), json);
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
                const r = await fetch('http://127.0.0.1:6277/health');
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
pub async fn get_all_rss_feeds(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse> {
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

pub async fn get_audit_logs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AuditLog>>> {
    let db = state.database.lock().await;
    let logs = db.get_audit_logs(100)?;
    Ok(Json(logs))
}

pub async fn get_shared_queue(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Returns a simplified view of the active queue for sharing
    let statuses = state.manager.all_progress().await;
    let simplified: Vec<serde_json::Value> = statuses.into_iter().map(|progress| {
        serde_json::json!({
            "id": progress.id,
            "filename": progress.filename,
            "bytes_downloaded": progress.bytes_downloaded,
            "total_bytes": progress.total_bytes,
            "progress_fraction": progress.progress_fraction,
            "state": progress.state,
        })
    }).collect();
    
    Json(simplified)
}

// ─── GET /api/v1/config/export ────────────────────────────────────────────────
pub async fn export_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    Json(config)
}

// ─── POST /api/v1/config/import ───────────────────────────────────────────────
pub async fn import_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<vajra_protocol::DaemonConfig>,
) -> Result<impl IntoResponse> {
    // 1. Update the in-memory config
    *state.config.write().await = body.clone();

    // 2. Persist to DB settings
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
    db.save_settings(&s).map_err(|e| DaemonError::Internal(e.to_string()))?;

    // 3. Update the download manager queue settings
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

    // 4. Persist to config file
    if let Ok(json) = serde_json::to_string_pretty(&body) {
        let _ = std::fs::write(vajra_protocol::config_path(), json);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
// ─── POST /api/v1/downloads/:id/preview ───────────────────────────────────────
pub async fn preview_download(
    Path(id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse> {
    use std::io::{Read, Write};

    let progress = state.manager.progress(id).await
        .ok_or_else(|| DaemonError::NotFound(id))?;

    let src_path = std::path::Path::new(&progress.dest_path);
    if !src_path.exists() {
        return Err(DaemonError::BadRequest("Download file does not exist yet".to_string()));
    }

    // Determine target preview path in temporary directory
    let filename = src_path.file_name()
        .ok_or_else(|| DaemonError::Internal("Invalid filename".to_string()))?;
    
    let temp_dir = std::env::temp_dir();
    let preview_path = temp_dir.join(format!("preview_{}", filename.to_string_lossy()));

    // Copy the partial file (up to the current downloaded bytes size to avoid copy bloat of huge unallocated files)
    let mut src_file = std::fs::File::open(src_path)
        .map_err(|e| DaemonError::Internal(e.to_string()))?;
    let mut dst_file = std::fs::File::create(&preview_path)
        .map_err(|e| DaemonError::Internal(e.to_string()))?;
    
    let copy_limit = progress.bytes_downloaded.min(10 * 1024 * 1024).max(1024);
    let mut buffer = vec![0u8; 8192];
    let mut total_copied = 0;
    
    while total_copied < copy_limit {
        let to_read = (copy_limit - total_copied).min(buffer.len() as u64) as usize;
        let read = src_file.read(&mut buffer[..to_read])
            .map_err(|e| DaemonError::Internal(e.to_string()))?;
        if read == 0 {
            break;
        }
        dst_file.write_all(&buffer[..read])
            .map_err(|e| DaemonError::Internal(e.to_string()))?;
        total_copied += read as u64;
    }

    // Open the preview file with the default system application (non-blocking)
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &preview_path.to_string_lossy()])
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
