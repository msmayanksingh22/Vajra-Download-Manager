use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use md5::Md5;
use sha2::{Digest, Sha256};

pub async fn verify_hash(filepath: &Path, expected_hash: &str) -> anyhow::Result<bool> {
    let expected = expected_hash.trim().to_lowercase();
    let is_sha256 = expected.starts_with("sha256:") || expected.len() == 64;
    let expected_hex = expected.replace("sha256:", "").replace("md5:", "");

    // Run hash computation in a blocking thread to avoid starving the async executor
    let filepath_clone = filepath.to_path_buf();
    let computed_hex = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let file = File::open(&filepath_clone)?;
        let mut reader = BufReader::with_capacity(1024 * 1024, file); // 1MB buffer

        if is_sha256 {
            let mut hasher = Sha256::new();
            let mut buffer = [0; 8192];
            loop {
                let n = reader.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        } else {
            let mut hasher = Md5::new();
            let mut buffer = [0; 8192];
            loop {
                let n = reader.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
    })
    .await??;

    Ok(computed_hex == expected_hex)
}

pub async fn auto_extract(filepath: &Path) -> anyhow::Result<PathBuf> {
    let filepath_clone = filepath.to_path_buf();

    // Spawn extraction in a blocking task
    tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
        let ext = filepath_clone
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut parent_dir = filepath_clone
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let file_stem = filepath_clone.file_stem().unwrap_or_default();
        parent_dir.push(file_stem);

        if ext == "zip" {
            let file = File::open(&filepath_clone)?;
            let mut archive = zip::ZipArchive::new(file)?;
            std::fs::create_dir_all(&parent_dir)?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = match file.enclosed_name() {
                    Some(path) => parent_dir.join(path),
                    None => continue,
                };

                if (*file.name()).ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
            Ok(parent_dir)
        } else if ext == "7z" {
            std::fs::create_dir_all(&parent_dir)?;
            sevenz_rust::decompress_file(&filepath_clone, &parent_dir)
                .map_err(|e| anyhow::anyhow!("7z extraction failed: {}", e))?;
            Ok(parent_dir)
        } else if ext == "rar" {
            std::fs::create_dir_all(&parent_dir)?;
            let mut archive = unrar::Archive::new(&filepath_clone)
                .open_for_processing()
                .map_err(|e| anyhow::anyhow!("RAR initialization failed: {}", e))?;
            while let Some(header) = archive
                .read_header()
                .map_err(|e| anyhow::anyhow!("RAR header error: {}", e))?
            {
                archive = if header.entry().is_file() {
                    header
                        .extract_to(&parent_dir)
                        .map_err(|e| anyhow::anyhow!("RAR extraction failed: {}", e))?
                } else {
                    header
                        .skip()
                        .map_err(|e| anyhow::anyhow!("RAR skip failed: {}", e))?
                };
            }
            Ok(parent_dir)
        } else {
            anyhow::bail!("Unsupported archive format: {}", ext);
        }
    })
    .await?
}

pub async fn run_post_processing_script(
    script_path: &Path,
    downloaded_file: &Path,
) -> anyhow::Result<()> {
    let ext = script_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    // NOTE: -ExecutionPolicy Bypass is intentionally NOT used here.
    // Bypassing the system execution policy would allow an attacker who can supply a script
    // path (e.g., via a crafted intercept request) to run arbitrary unsigned PowerShell code.
    // The system policy (RemoteSigned / AllSigned) acts as a last line of defence.
    // The script_path is passed as a literal argument to the process, not shell-interpolated,
    // so special characters in the path cannot cause command injection.
    let mut cmd = if ext == "ps1" {
        let mut c = tokio::process::Command::new("powershell");
        c.arg("-NonInteractive").arg("-File").arg(script_path);
        c.stdin(std::process::Stdio::null());
        c
    } else {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(script_path);
        c.stdin(std::process::Stdio::null());
        c
    };

    cmd.arg(downloaded_file);

    let status = cmd.status().await?;
    if !status.success() {
        anyhow::bail!("Script exited with status: {}", status);
    }

    Ok(())
}

pub async fn run_antivirus_scan(
    av_path: &Path,
    av_args: &[String],
    downloaded_file: &Path,
) -> anyhow::Result<()> {
    // Build the command using the configured AV path
    let mut cmd = tokio::process::Command::new(av_path);
    cmd.stdin(std::process::Stdio::null());

    // Inject arguments. Replace special token "{FILE}" with the actual path,
    // otherwise append the file path to the end if not explicitly placed.
    let mut file_added = false;
    for arg in av_args {
        if arg == "{FILE}" {
            cmd.arg(downloaded_file);
            file_added = true;
        } else {
            cmd.arg(arg);
        }
    }

    if !file_added {
        cmd.arg(downloaded_file);
    }

    // Run AV scanner invisibly
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().await?;

    // Most AV scanners return 0 for clean, non-zero for infected or error
    if !output.status.success() {
        anyhow::bail!(
            "Antivirus detected a threat or failed (exit code {}). Output: {:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    Ok(())
}

/// Generates a thumbnail for a video file using ffmpeg.
pub async fn generate_thumbnail(video_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.arg("-y") // Overwrite output files
        .arg("-i")
        .arg(video_path)
        .arg("-ss")
        .arg("00:00:05") // Take frame at 5 seconds
        .arg("-vframes")
        .arg("1")
        .arg("-vf")
        .arg("scale=320:-1") // Scale width to 320, maintain aspect ratio
        .arg(output_path)
        .stdin(std::process::Stdio::null());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "ffmpeg failed to generate thumbnail (exit code {}). Output: {:?}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
