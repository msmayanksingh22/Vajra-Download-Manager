//! axum Router assembly for the Vajra daemon REST API.

use std::sync::Arc;

use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use dav_server::{localfs::LocalFs, DavHandler};

use crate::{api::handlers, AppState};

/// Auth middleware: requires Bearer token on all API routes except /health and /setup.
async fn auth_middleware(
    state: Arc<AppState>,
    req: Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Skip auth for health check and setup page
    let path = req.uri().path();
    if path == "/health" || path == "/setup" {
        return Ok(next.run(req).await);
    }

    // Check for Bearer token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    // Read the expected token from config
    let expected_token = state.config.read().await.api_token.clone();

    // If no token is configured, skip auth (first-run / dev mode)
    let Some(expected) = expected_token else {
        return Ok(next.run(req).await);
    };

    if let Some(auth) = auth_header {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            if token == expected {
                return Ok(next.run(req).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

pub async fn build(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .merge(SwaggerUi::new("/api/v1/docs").url("/api-docs/openapi.json", crate::api::schema::ApiDoc::openapi()))
        // Downloads
        .route("/downloads", post(handlers::add_download))
        .route("/downloads", get(handlers::list_downloads))
        .route("/downloads/:id", get(handlers::get_download))
        .route("/downloads/:id", patch(handlers::patch_download))
        .route("/downloads/:id", delete(handlers::delete_download))
        .route("/downloads/:id/events", get(handlers::download_events))
        .route("/downloads/:id/preview", post(handlers::preview_download))
        // Import
        .route("/import/ef2", post(crate::api::import::import_ef2_handler))
        // Decryption container
        .route("/decrypt", post(crate::api::import::decrypt_handler))
        // Global SSE stream
        .route("/events", get(handlers::global_events))
        // Global WebSocket stream
        .route("/ws", get(handlers::ws_handler))
        // Inspect (pre-flight probe)
        .route("/inspect", post(handlers::inspect_url))
        // Intercept (from extension)
        .route("/intercept", post(handlers::intercept_url))
        // Stats
        .route("/stats", get(handlers::stats))
        // Spider
        .route("/spider", get(crate::api::spider::run_spider))
        // Config
        .route("/config", get(handlers::get_config))
        .route("/config", patch(handlers::patch_config))
        .route("/config/export", get(handlers::export_config))
        .route("/config/import", post(handlers::import_config))
        // Vault
        .route("/vault", get(handlers::get_vault_credentials))
        .route("/vault", post(handlers::add_vault_credential))
        .route("/vault/:id", delete(handlers::delete_vault_credential))
        // RSS
        .route("/rss", get(handlers::get_all_rss_feeds))
        .route("/rss", post(handlers::add_rss_feed))
        .route("/rss/:id", delete(handlers::delete_rss_feed))
        // Collaboration
        .route("/audit", get(handlers::get_audit_logs))
        .route("/shared/queue", get(handlers::get_shared_queue))
        // Auth middleware on all /api/v1 routes
        .route_layer(middleware::from_fn({
            let state = state.clone();
            move |req: Request, next: middleware::Next| {
                let state = state.clone();
                async move { auth_middleware(state, req, next).await }
            }
        }));

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            |origin: &axum::http::HeaderValue, _request_parts: &axum::http::request::Parts| {
                if let Ok(o) = origin.to_str() {
                    o.starts_with("http://localhost")
                        || o.starts_with("http://127.0.0.1")
                        || o.starts_with("tauri://")
                        || o.starts_with("https://tauri.localhost")
                        || o.starts_with("chrome-extension://")
                        || o.starts_with("moz-extension://")
                } else {
                    false
                }
            },
        ))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    let output_dir = {
        let config_lock = state.config.read().await;
        config_lock.default_output_dir.clone()
    };
    let dav_server = DavHandler::builder()
        .strip_prefix("/webdav")
        .filesystem(LocalFs::new(output_dir, false, false, false))
        .locksystem(dav_server::memls::MemLs::new())
        .build_handler();
    let dav_server = Arc::new(dav_server);

    let dav_router = Router::new()
        .route("/*path", axum::routing::any(crate::api::webdav::webdav_handler))
        .route("/", axum::routing::any(crate::api::webdav::webdav_handler))
        .with_state(dav_server);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/setup", get(handlers::browser_setup))
        .nest("/webdav", dav_router)
        .nest("/api/v1", api)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ─── OpenAPI Documentation ────────────────────────────────────────────────────

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        handlers::health,
        handlers::add_download,
    ),
    components(
        schemas(
            vajra_protocol::AddDownloadRequest,
            vajra_protocol::AddDownloadResponse,
            vajra_protocol::DownloadList,
            vajra_protocol::DownloadInfo,
            vajra_protocol::DownloadStatus,
            vajra_protocol::PatchDownloadRequest,
            vajra_protocol::DownloadAction,
            vajra_protocol::InspectRequest,
            vajra_protocol::InspectResponse,
            vajra_protocol::ProxyConfig,
            vajra_protocol::CategoryRule,
            vajra_protocol::S3Config,
            vajra_protocol::DaemonConfig,
            vajra_protocol::PostQueueAction,
            vajra_protocol::DuplicateAction,
            vajra_protocol::StatsResponse,
            vajra_protocol::QueueType,
            vajra_protocol::Priority,
            vajra_protocol::AddVaultCredentialRequest,
            vajra_protocol::VaultCredentialResponse,
            vajra_protocol::AddRssFeedRequest,
            vajra_protocol::RssFeed,
        )
    ),
    tags(
        (name = "Vajra", description = "Vajra Download Manager API")
    )
)]
struct ApiDoc;
