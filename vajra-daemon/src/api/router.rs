//! axum Router assembly for the Vajra daemon REST API.

use std::sync::Arc;

use axum::{
    extract::Request,
    http::Method,
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{api::handlers, AppState};

pub async fn build(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .merge(SwaggerUi::new("/api/v1/docs").url("/api-docs/openapi.json", crate::api::schema::ApiDoc::openapi()))
        // Downloads
        .route("/downloads", post(handlers::add_download))
        .route("/downloads", get(handlers::list_downloads))
        .route("/downloads/bulk-action", post(handlers::bulk_action))
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
                async move { crate::api::auth::auth_middleware(state, req, next).await }
            }
        }));

    let state_cors = state.clone();
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin: &axum::http::HeaderValue, _request_parts: &axum::http::request::Parts| {
                if let Ok(o) = origin.to_str() {
                    if let Ok(config) = state_cors.config.try_read() {
                        crate::api::auth::is_allowed_origin(o, &config.allowed_extension_ids)
                    } else {
                        crate::api::auth::is_allowed_origin(o, &[])
                    }
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

    let dav_router = Router::new()
        .route(
            "/*path",
            axum::routing::any(crate::api::webdav::webdav_handler),
        )
        .route("/", axum::routing::any(crate::api::webdav::webdav_handler))
        .layer(middleware::from_fn({
            let state = state.clone();
            move |req: Request, next: middleware::Next| {
                let state = state.clone();
                async move { crate::api::auth::auth_middleware(state, req, next).await }
            }
        }))
        .with_state(state.clone());

    let base_router = Router::new()
        .route("/health", get(handlers::health))
        .route("/setup", get(handlers::browser_setup))
        .nest("/api/v1", api)
        .nest("/webdav", dav_router)
        .merge(SwaggerUi::new("/swagger-ui").url(
            "/api-docs/openapi.json",
            crate::api::schema::ApiDoc::openapi(),
        ));

    base_router
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024))
        .with_state(state)
}
