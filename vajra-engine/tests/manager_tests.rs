use std::path::PathBuf;

use uuid::Uuid;
use vajra_engine::{download_task::DownloadRequest, queue::QueueSettings, DownloadManager};
use vajra_protocol::{Priority, QueueType};

fn make_req(url: &str, priority: Priority) -> DownloadRequest {
    DownloadRequest {
        url: url.to_string(),
        mirrors: vec![],
        dest_dir: PathBuf::from("/tmp"),
        filename: None,
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
        priority,
        tags: vec![],
        ..Default::default()
    }
}

#[tokio::test]
async fn test_manager_queue_ordering() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let settings = QueueSettings {
        max_concurrent: 1, // Only process one at a time
        ..Default::default()
    };
    let manager = DownloadManager::new(settings, 0);

    let req1 = make_req("http://example.com/1", Priority::Normal);
    let req2 = make_req("http://example.com/2", Priority::High);

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    manager.add_with_id(id1, req1).await;
    manager.add_with_id(id2, req2).await;

    let entries = manager.all_progress().await;
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn test_manager_fap() {
    let settings = QueueSettings {
        fap_enabled: true,
        fap_quota_bytes: 1024, // 1KB quota
        max_concurrent: 2,
        ..Default::default()
    };
    let _manager = DownloadManager::new(settings, 0);
}

#[tokio::test]
async fn test_manager_lifecycle() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::env::set_var("VAJRA_DATA_DIR", temp_dir.path());

    let settings = QueueSettings {
        max_concurrent: 2,
        ..Default::default()
    };
    let manager = DownloadManager::new(settings, 0);

    let req = make_req("http://example.com/lifecycle", Priority::Normal);
    let id = Uuid::new_v4();

    // 1. Add
    manager.add_with_id(id, req).await;

    // Wait for it to show up
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let progress = manager.progress(id).await.expect("Task not found");
    assert_eq!(progress.url, "http://example.com/lifecycle");

    // 2. Pause
    manager.pause(id).await;
    // (It might take a tick for state to reflect Pause/Pausing, but we can verify it's no longer actively pulling from queue to new state)

    // 3. Resume
    manager.resume(id).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 4. Cancel
    manager.cancel(id).await;
    let entries = manager.all_progress().await;
    assert!(
        entries.iter().all(|e| e.id != id),
        "Task should be removed from manager"
    );
}

#[tokio::test]
async fn test_manager_retry_state_matrix() {
    use vajra_engine::download_task::{DownloadTask, TaskState};

    let settings = QueueSettings {
        max_concurrent: 0, // Keep in queued state
        ..Default::default()
    };
    let manager = DownloadManager::new(settings, 0);
    let id_failed = Uuid::new_v4();
    let id_completed = Uuid::new_v4();
    let req = make_req("http://example.com/test", Priority::Normal);

    // Add restored failed task
    let task_failed = DownloadTask::new_restored(
        id_failed,
        req.clone(),
        TaskState::Failed,
        100,
        1000,
        "failed.bin".into(),
        "/tmp/failed.bin".into(),
        Some("Network dropped".into()),
    );
    manager
        .add_restored(id_failed, req.clone(), task_failed)
        .await;

    // Add restored completed task
    let task_completed = DownloadTask::new_restored(
        id_completed,
        req.clone(),
        TaskState::Completed,
        1000,
        1000,
        "complete.bin".into(),
        "/tmp/complete.bin".into(),
        None,
    );
    manager
        .add_restored(id_completed, req.clone(), task_completed)
        .await;

    // 1. Retry on Failed MUST succeed
    let retry_res = manager.retry_task(id_failed).await;
    assert!(retry_res.is_ok(), "Retry on failed task must succeed");

    // 2. Retry on Completed MUST be rejected with invalid_state
    let retry_completed_res = manager.retry_task(id_completed).await;
    assert!(
        matches!(
            retry_completed_res,
            Err(vajra_engine::QueueActionError::InvalidState(_))
        ),
        "Retry on completed task must be rejected"
    );

    // 3. Resume on Completed MUST be rejected
    let resume_completed_res = manager.resume_task(id_completed).await;
    assert!(
        matches!(
            resume_completed_res,
            Err(vajra_engine::QueueActionError::InvalidState(_))
        ),
        "Resume on completed task must be rejected"
    );

    // 4. Pause on non-existent task returns NotFound
    let pause_missing = manager.pause_task(Uuid::new_v4()).await;
    assert!(
        matches!(pause_missing, Err(vajra_engine::QueueActionError::NotFound)),
        "Pause on missing task must return NotFound"
    );
}
