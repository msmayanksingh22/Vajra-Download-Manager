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
        hash_result: None,
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

use utoipa::OpenApi;
use vajra_protocol::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::handlers::health,
        crate::api::handlers::add_download,
        crate::api::handlers::list_downloads,
        crate::api::handlers::get_download,
        crate::api::handlers::patch_download,
        crate::api::handlers::delete_download,
        crate::api::handlers::stats,
    ),
    components(
        schemas(
            AddDownloadRequest,
            AddDownloadResponse,
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
        )
    ),
    tags(
        (name = "vajra", description = "Vajra Download Manager API")
    )
)]
pub struct ApiDoc;
