use std::sync::Arc;

use axum::{extract::State, Json};
use vajra_protocol::{AddDownloadRequest, ImportEf2Request, ImportEf2Response};

use crate::{api::handlers::add_download, AppState, DaemonError};

pub async fn import_ef2_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ImportEf2Request>,
) -> Result<Json<ImportEf2Response>, DaemonError> {
    let mut imported = 0;
    let mut errors = Vec::new();

    for line in payload.content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split('<').map(|s| s.trim());
        if let Some(url) = parts.next() {
            if url.starts_with("http://") || url.starts_with("https://") {
                let mut req = AddDownloadRequest {
                    url: url.to_string(),
                    output_dir: None,
                    filename: None,
                    headers: std::collections::HashMap::new(),
                    expected_hash: None,
                    use_http3: false,
                    max_connections: None,
                    speed_limit_bps: None,
                    priority: vajra_protocol::Priority::Normal,
                    schedule_at: None,
                    use_ytdlp: false,
                    ytdlp_format: None,
                    ytdlp_subtitles: false,
                    ytdlp_playlist: false,
                    auto_extract: false,
                    post_processing_script: None,
                    queue_type: None,
                    sync_interval_secs: None,
                    tags: None,
                };

                if let Some(path_or_file) = parts.next() {
                    if !path_or_file.is_empty() && path_or_file != "-" {
                        let path = std::path::Path::new(path_or_file);
                        if let Some(file_name) = path.file_name() {
                            req.filename = Some(file_name.to_string_lossy().to_string());
                        }
                    }
                }

                match add_download(State(state.clone()), Json(req)).await {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("Failed to add {url}: {:?}", e)),
                }
            }
        }
    }

    Ok(Json(ImportEf2Response {
        imported_count: imported,
        errors,
    }))
}

pub async fn decrypt_handler(
    State(_state): State<Arc<AppState>>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Vec<String>>, DaemonError> {
    if let Some(field) = multipart.next_field().await.map_err(|e| DaemonError::BadRequest(e.to_string()))? {
        let file_bytes = field.bytes().await.map_err(|e| DaemonError::BadRequest(e.to_string()))?;
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("temp.dlc");
        std::fs::write(&temp_file, &file_bytes).map_err(|e| DaemonError::Internal(e.to_string()))?;
        
        let client = reqwest::Client::new();
        let links = vajra_engine::decryption::decrypt_dlc_file(&client, "https://dcrypt.it/api/decrypt", &temp_file).await
            .map_err(|e| DaemonError::Internal(e.to_string()))?;
            
        let _ = std::fs::remove_file(temp_file);
        return Ok(Json(links));
    }
    Err(DaemonError::BadRequest("Missing file".to_string()))
}
