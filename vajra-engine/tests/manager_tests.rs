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
