use std::path::Path;

use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    config::{Credentials, Region},
    primitives::ByteStream,
    Client,
};
use vajra_protocol::DaemonConfig;

/// Upload a file to an S3 bucket configured in DaemonConfig.
pub async fn upload_file_to_s3(file_path: &Path, config: &DaemonConfig) -> Result<()> {
    if !config.s3_enabled {
        return Ok(());
    }

    let bucket = config
        .s3_bucket
        .as_deref()
        .context("S3 bucket not configured")?;

    let region = config
        .s3_region
        .clone()
        .unwrap_or_else(|| "us-east-1".to_string());

    let mut config_builder =
        aws_config::defaults(BehaviorVersion::latest()).region(Region::new(region));

    if let (Some(access_key), Some(secret_key)) = (&config.s3_access_key, &config.s3_secret_key) {
        config_builder = config_builder.credentials_provider(Credentials::new(
            access_key.clone(),
            secret_key.clone(),
            None,
            None,
            "vajra",
        ));
    }

    if let Some(endpoint) = &config.s3_endpoint {
        config_builder = config_builder.endpoint_url(endpoint.clone());
    }

    let sdk_config = config_builder.load().await;
    let client = Client::new(&sdk_config);

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file name for S3 upload")?;

    let body = ByteStream::from_path(file_path).await?;

    tracing::info!("Uploading {} to S3 bucket {}", file_name, bucket);

    client
        .put_object()
        .bucket(bucket)
        .key(file_name)
        .body(body)
        .send()
        .await
        .context("Failed to upload to S3")?;

    tracing::info!("Successfully uploaded {} to S3", file_name);

    if config.s3_delete_local {
        tracing::info!(
            "Deleting local file {} after S3 upload",
            file_path.display()
        );
        std::fs::remove_file(file_path)?;
    }

    Ok(())
}
