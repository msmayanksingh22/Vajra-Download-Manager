use std::{path::Path, process::Command};

use anyhow::{Context, Result};

/// Auto-muxes a list of `.ts` files into a single `.mp4` file using FFmpeg.
/// Expects the input files to be written sequentially in an FFmpeg concat list format.
pub async fn mux_ts_files(concat_list_path: &Path, output_path: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y") // Overwrite output files without asking
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(concat_list_path)
        .arg("-c")
        .arg("copy")
        .arg(output_path)
        .output()
        .context("Failed to execute ffmpeg command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg muxing failed:\n{}", stderr);
    }

    Ok(())
}

/// Muxes separate video and audio streams into a single `.mp4` file.
pub async fn mux_video_audio(
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("copy")
        .arg(output_path)
        .output()
        .context("Failed to execute ffmpeg command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg muxing failed:\n{}", stderr);
    }

    Ok(())
}
