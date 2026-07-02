use std::path::{Path, PathBuf};

use tracing::{info, warn};

/// Result of content pipeline processing
#[derive(Debug)]
pub struct ProcessedContent {
    pub original_path: PathBuf,
    pub thumbnail_path: Option<PathBuf>,
    pub metadata_path: Option<PathBuf>,
}

/// Run the post-download content pipeline based on file extension/type.
pub async fn run_pipeline(filepath: &Path) -> anyhow::Result<ProcessedContent> {
    let mut result = ProcessedContent {
        original_path: filepath.to_path_buf(),
        thumbnail_path: None,
        metadata_path: None,
    };

    let mut ext = filepath
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // MIME sniffing fallback if extension is missing
    if ext.is_empty() {
        if let Ok(Some(inferred)) = infer::get_from_path(filepath) {
            ext = inferred.extension().to_string();
            info!(
                "MIME sniffing detected extension: {} for {:?}",
                ext, filepath
            );
        }
    }

    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "webm" => {
            // Generate video thumbnail
            let mut thumb_path = filepath.to_path_buf();
            thumb_path.set_extension("thumb.jpg");

            info!("Generating video thumbnail for {:?}", filepath);
            if let Err(e) = crate::post_processing::generate_thumbnail(filepath, &thumb_path).await
            {
                warn!("Failed to generate video thumbnail: {}", e);
            } else {
                result.thumbnail_path = Some(thumb_path);
            }
        }
        "jpg" | "jpeg" | "png" | "webp" => {
            // Simple image thumbnail generation (using ffmpeg as a universal fallback)
            let mut thumb_path = filepath.to_path_buf();
            thumb_path.set_extension("thumb.jpg");

            info!("Generating image thumbnail for {:?}", filepath);
            let mut cmd = tokio::process::Command::new("ffmpeg");
            cmd.arg("-y")
                .arg("-i")
                .arg(filepath)
                .arg("-vf")
                .arg("scale=320:-1")
                .arg("-vframes")
                .arg("1")
                .arg(&thumb_path)
                .stdin(std::process::Stdio::null());

            #[cfg(windows)]
            {
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }

            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    result.thumbnail_path = Some(thumb_path);
                } else {
                    warn!("Image thumbnail generation failed");
                }
            }
        }
        "pdf" => {
            // We could extract text or render a thumbnail using Ghostscript/pdftoppm,
            // but for now we just flag it.
            info!("PDF processing placeholder for {:?}", filepath);
        }
        _ => {
            // No pipeline configured for this extension
        }
    }

    // Execute WASM plugins
    let plugins_dir = vajra_protocol::app_data_dir().join("plugins");
    if !plugins_dir.exists() {
        let _ = std::fs::create_dir_all(&plugins_dir);
    }

    if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                info!("Executing WebAssembly plugin: {:?}", path.file_name());
                let parent_dir = filepath
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .to_string();

                // Set up manifest with sandboxed directory access & memory/time constraints
                let manifest = extism::Manifest::new([extism::Wasm::file(&path)])
                    .with_allowed_path(parent_dir.clone(), parent_dir)
                    .with_memory_max(1024) // 64 MB
                    .with_timeout(std::time::Duration::from_secs(10));

                match extism::Plugin::new(&manifest, [], true) {
                    Ok(mut plugin) => {
                        let input = filepath.to_string_lossy().to_string();
                        match plugin.call::<&str, &str>("on_download_complete", &input) {
                            Ok(out) => info!(
                                "Plugin {:?} run successful. Output: {}",
                                path.file_name(),
                                out
                            ),
                            Err(e) => {
                                warn!("Plugin {:?} execution failed: {:?}", path.file_name(), e)
                            }
                        }
                    }
                    Err(e) => warn!("Failed to load WASM plugin {:?}: {:?}", path.file_name(), e),
                }
            }
        }
    }

    Ok(result)
}
