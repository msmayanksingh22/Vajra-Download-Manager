//! Link Decryption Module
//!
//! Provides support for parsing and decrypting DLC and RSDF link containers
//! via external APIs.

use std::path::Path;
use reqwest::Client;

/// Decrypt a DLC file using an external decrypter API (e.g. dcrypt.it).
pub async fn decrypt_dlc_file(client: &Client, api_url: &str, file_path: &Path) -> anyhow::Result<Vec<String>> {
    let file_bytes = tokio::fs::read(file_path).await?;
    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name("file.dlc")
        .mime_str("application/octet-stream")?;
    let form = reqwest::multipart::Form::new().part("dlcfile", part);

    let res = client
        .post(api_url)
        .multipart(form)
        .send()
        .await?;

    if !res.status().is_success() {
        anyhow::bail!("DLC decryption API returned status: {}", res.status());
    }

    // Example response structure: { "success": { "links": ["url1", "url2"] } }
    #[derive(serde::Deserialize)]
    struct DcryptResponse {
        success: Option<DcryptSuccess>,
    }
    
    #[derive(serde::Deserialize)]
    struct DcryptSuccess {
        links: Vec<String>,
    }

    let parsed: DcryptResponse = res.json().await?;
    if let Some(success) = parsed.success {
        Ok(success.links)
    } else {
        anyhow::bail!("Failed to parse DLC links from response");
    }
}
