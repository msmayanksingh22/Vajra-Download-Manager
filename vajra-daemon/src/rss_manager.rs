use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{error, info, debug};
use uuid::Uuid;

use crate::AppState;
use vajra_engine::download_task::DownloadRequest;

pub struct RssManager;

impl RssManager {
    pub fn start(state: Arc<AppState>) {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60 * 60)); // Check every hour
            let client = reqwest::Client::new();

            loop {
                interval.tick().await;
                debug!("Running RSS feed check...");

                let feeds = {
                    let db = state.database.lock().await;
                    match db.get_all_rss_feeds() {
                        Ok(f) => f,
                        Err(e) => {
                            error!("Failed to fetch RSS feeds: {}", e);
                            continue;
                        }
                    }
                };

                for feed in feeds {
                    debug!("Checking RSS feed: {}", feed.title);
                    match client.get(&feed.url).send().await {
                        Ok(response) => {
                            if let Ok(bytes) = response.bytes().await {
                                if let Ok(channel) = rss::Channel::read_from(&bytes[..]) {
                                    for item in channel.items() {
                                        if let Some(enclosure) = item.enclosure() {
                                            let guid = item.guid().map(|g| g.value()).unwrap_or(enclosure.url());
                                            
                                            let db = state.database.lock().await;
                                            match db.rss_item_exists(&feed.id, guid) {
                                                Ok(false) => {
                                                    // Need to download this!
                                                    let download_id = Uuid::new_v4();
                                                    let req = DownloadRequest {
                                                        url: enclosure.url().to_string(),
                                                        mirrors: vec![],
                                                        dest_dir: std::path::PathBuf::from(state.config.read().await.default_output_dir.clone()),
                                                        filename: item.title().map(|t| format!("{}.mp3", t.replace("/", "_"))),
                                                        timeout_secs: None,
                                                        connect_timeout_secs: None,
                                                        max_connections: 8,
                                                        speed_limit: 0,
                                                        throttle: None,
                                                        delete_on_failure: false,
                                                        use_http3: false,
                                                        queue_type: Default::default(),
                                                        sync_interval_secs: 5,
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
                                                        expected_hash: None,
                                                        auto_extract: false,
                                                        post_processing_script: None,
                                                        av_scan_path: None,
                                                        av_scan_args: vec![],
                                                        schedule_at: None,
                                                        daemon_config: None,
                                                        priority: vajra_protocol::Priority::Normal,
                                                        tags: vec!["rss".to_string(), feed.title.clone()],
                                                        tcp_multiplexing_opt: false,
                                                        adaptive_chunk_v2: false,
                                                    };
                                                    
                                                    // Insert to db to prevent duplicate downloads
                                                    if let Err(e) = db.add_rss_item(
                                                        &Uuid::new_v4().to_string(),
                                                        &feed.id,
                                                        guid,
                                                        Some(&download_id.to_string())
                                                    ) {
                                                        error!("Failed to add RSS item to DB: {}", e);
                                                    }
                                                    
                                                    drop(db); // Drop lock before adding to manager
                                                    state.manager.add_with_id(download_id, req).await;
                                                    info!("Added RSS enclosure to download queue: {}", enclosure.url());
                                                }
                                                Ok(true) => {
                                                    // Already downloaded
                                                    debug!("RSS item already processed: {}", guid);
                                                }
                                                Err(e) => {
                                                    error!("Failed to check RSS item in DB: {}", e);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    error!("Failed to parse RSS feed: {}", feed.url);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to fetch RSS feed {}: {}", feed.url, e);
                        }
                    }
                }
            }
        });
    }
}
