//! API-local schema helpers — conversions from engine types to REST response types.

use vajra_engine::download_task::{DownloadProgress, TaskState};
use vajra_protocol::DownloadInfo;

/// Convert an engine `DownloadProgress` snapshot into the REST `DownloadInfo` type.
pub fn progress_to_info(p: &DownloadProgress) -> DownloadInfo {
    let status = state_to_status(&p.state);
    let completed_at = if p.state == TaskState::Completed || p.state == TaskState::Failed {
        Some(chrono::Utc::now().timestamp())
    } else {
        None
    };

    DownloadInfo {
        id: p.id,
        priority: Default::default(),
        status,
        url: p.url.clone(),
        output_path: if p.dest_path.is_empty() {
            None
        } else {
            Some(p.dest_path.clone())
        },
        filename: p.filename.clone(),
        total_bytes: if p.total_bytes > 0 {
            Some(p.total_bytes)
        } else {
            None
        },
        bytes_done: p.bytes_downloaded,
        speed_bps: p.speed_bps,
        eta_seconds: if p.eta_secs > 0 {
            Some(p.eta_secs)
        } else {
            None
        },
        progress_pct: if p.state == TaskState::Completed {
            100.0
        } else {
            (p.progress_fraction * 100.0 * 10.0).round() / 10.0
        },
        connections_active: p.segments.len() as u8,
        segments: p.segments.clone(),
        hash_result: p.hash_result.clone(),
        expected_hash: p.expected_hash.clone(),
        actual_hash: p.hash_result.as_ref().map(|h| h.computed.clone()),
        hash_algorithm: p.hash_result.as_ref().map(|h| h.algorithm.clone()),
        resume_supported: p.resume_supported,
        created_at: chrono::Utc::now().timestamp(),
        started_at: None,
        completed_at,
        error: p.error.clone(),
        speed_history: vec![],
        queue_type: match p.queue_type {
            vajra_engine::download_task::QueueType::Standard => "Standard".to_string(),
            vajra_engine::download_task::QueueType::Synchronization => {
                "Synchronization".to_string()
            }
        },
        sync_interval_secs: p.sync_interval_secs,
        tags: p.tags.clone(),
        speed_limit_bps: Some(p.speed_limit_bps),
    }
}

pub fn state_to_status(state: &TaskState) -> vajra_protocol::DownloadStatus {
    match state {
        TaskState::Queued => vajra_protocol::DownloadStatus::Idle,
        TaskState::FetchingMeta => vajra_protocol::DownloadStatus::Connecting,
        TaskState::SolvingCaptcha => vajra_protocol::DownloadStatus::Connecting,
        TaskState::Allocating => vajra_protocol::DownloadStatus::Connecting,
        TaskState::Downloading => vajra_protocol::DownloadStatus::Downloading,
        TaskState::Pausing => vajra_protocol::DownloadStatus::Paused,
        TaskState::Paused => vajra_protocol::DownloadStatus::Paused,
        TaskState::Verifying => vajra_protocol::DownloadStatus::Verifying,
        TaskState::Completed => vajra_protocol::DownloadStatus::Completed,
        TaskState::Failed => vajra_protocol::DownloadStatus::Failed,
        TaskState::Cancelled => vajra_protocol::DownloadStatus::Failed,
    }
}

pub fn state_str(state: &TaskState) -> &'static str {
    match state {
        TaskState::Queued => "queued",
        TaskState::FetchingMeta => "fetching_meta",
        TaskState::SolvingCaptcha => "solving_captcha",
        TaskState::Allocating => "allocating",
        TaskState::Downloading => "downloading",
        TaskState::Pausing => "pausing",
        TaskState::Paused => "paused",
        TaskState::Verifying => "verifying",
        TaskState::Completed => "complete",
        TaskState::Failed => "failed",
        TaskState::Cancelled => "cancelled",
    }
}

use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};
use vajra_protocol::*;

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("Hex Token")
                        .description(Some(
                            "Vajra daemon authentication header: `Authorization: Bearer <token>`",
                        ))
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vajra Daemon API",
        version = "0.1.0",
        description = "Vajra Download Manager API"
    ),
    modifiers(&SecurityAddon),
    security(
        ("bearer_auth" = [])
    ),
    paths(
        crate::api::handlers::health,
        crate::api::handlers::add_download,
        crate::api::handlers::list_downloads,
        crate::api::handlers::bulk_action,
        crate::api::handlers::get_download,
        crate::api::handlers::patch_download,
        crate::api::handlers::delete_download,
        crate::api::handlers::preview_download,
        crate::api::handlers::inspect_url,
        crate::api::handlers::intercept_url,
        crate::api::handlers::stats,
        crate::api::handlers::get_config,
        crate::api::handlers::patch_config,
        crate::api::handlers::export_config,
        crate::api::handlers::import_config,
        crate::api::handlers::get_vault_credentials,
        crate::api::handlers::add_vault_credential,
        crate::api::handlers::delete_vault_credential,
        crate::api::handlers::add_rss_feed,
        crate::api::handlers::get_all_rss_feeds,
        crate::api::handlers::delete_rss_feed,
    ),
    components(
        schemas(
            AddDownloadRequest,
            AddDownloadResponse,
            BulkActionRequest,
            BulkAction,
            BulkActionResponse,
            BulkActionFailure,
            DownloadList,
            DownloadInfo,
            DownloadStatus,
            SegmentInfo,
            HashResult,
            DownloadProgressResponse,
            PatchDownloadRequest,
            DownloadAction,
            Priority,
            InspectRequest,
            InspectResponse,
            StatsResponse,
            QueueType,
            DaemonConfig,
            ProxyConfig,
            CategoryRule,
            S3Config,
            PostQueueAction,
            DuplicateAction,
            AddVaultCredentialRequest,
            VaultCredentialResponse,
            AddRssFeedRequest,
            RssFeed,
            ApiErrorResponse,
            ApiErrorDetail,
        )
    ),
    tags(
        (name = "vajra", description = "Vajra Download Manager API")
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use utoipa::OpenApi;

    use super::*;

    #[test]
    fn test_openapi_schema_generation() {
        let openapi = ApiDoc::openapi();
        assert_eq!(openapi.info.title, "Vajra Daemon API");
        assert_eq!(openapi.info.version, "0.1.0");

        let paths = &openapi.paths.paths;
        assert!(paths.contains_key("/api/v1/downloads"));
        assert!(paths.contains_key("/api/v1/downloads/{id}"));
        assert!(paths.contains_key("/api/v1/config"));
        assert!(paths.contains_key("/api/v1/config/import"));
        assert!(paths.contains_key("/api/v1/config/export"));
        assert!(paths.contains_key("/api/v1/downloads/{id}/preview"));
        assert!(paths.contains_key("/health"));
        assert!(paths.contains_key("/api/v1/stats"));

        let schemas = &openapi.components.as_ref().unwrap().schemas;
        assert!(schemas.contains_key("ApiErrorResponse"));
        assert!(schemas.contains_key("ApiErrorDetail"));
        assert!(schemas.contains_key("DaemonConfig"));
        assert!(schemas.contains_key("AddDownloadRequest"));

        let sec_schemes = &openapi.components.as_ref().unwrap().security_schemes;
        assert!(sec_schemes.contains_key("bearer_auth"));

        let json = openapi.to_json().unwrap();
        assert!(!json.is_empty());
    }
}
