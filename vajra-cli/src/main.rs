//! Vajra CLI — `vajra` command-line interface to the daemon REST API.
//!
//! Usage:
//!   vajra get <URL> [OPTIONS]
//!   vajra list [--status <status>]
//!   vajra pause <ID>
//!   vajra resume <ID>
//!   vajra cancel <ID>
//!   vajra stats
//!   vajra inspect <URL>
//!   vajra daemon   (start daemon if not running)

use std::{process, time::Duration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use vajra_protocol::{AddDownloadRequest, DownloadAction, PatchDownloadRequest, DEFAULT_PORT};

fn api(path: &str) -> String {
    let port = std::env::var("VAJRA_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    format!("http://127.0.0.1:{}/api/v1{}", port, path)
}

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "vajra",
    about = "Vajra download manager — CLI client",
    version,
    propagate_version = true,
    styles = clap_styles()
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add a new download
    #[command(alias = "add")]
    Get {
        url: String,
        /// Output directory (short: /d)
        #[arg(short, long)]
        out: Option<String>,
        /// Override filename (short: /f)
        #[arg(short, long)]
        filename: Option<String>,
        /// Max parallel connections (1-32)
        #[arg(short = 'c', long, default_value = "8")]
        connections: u32,
        /// Speed limit in bytes/sec (0 = unlimited)
        #[arg(long, default_value = "0")]
        limit: u64,
        /// Watch progress after adding
        #[arg(short, long)]
        watch: bool,
        /// Priority: high | normal | low  (short: /p)
        #[arg(short, long, default_value = "normal")]
        priority: String,
        /// Add to queue without auto-starting  (short: /q)
        #[arg(short = 'q', long)]
        queue_only: bool,
        /// Force yt-dlp for streaming URLs
        #[arg(long)]
        ytdlp: bool,
        /// Expected SHA-256 or MD5 hash (verified after download)
        #[arg(long)]
        hash: Option<String>,
        /// Quiet mode (no UI/output)
        #[arg(long)]
        quiet: bool,
        /// Auto-extract archive upon completion
        #[arg(short = 'x', long)]
        auto_extract: bool,
        /// Post-processing script to run after completion
        #[arg(long)]
        script: Option<String>,
        /// Schedule download at a specific Unix timestamp
        #[arg(long)]
        schedule: Option<i64>,
    },

    /// List downloads
    #[command(alias = "ls")]
    List {
        /// Filter by status: all | queued | downloading | paused | complete | failed
        #[arg(short, long, default_value = "all")]
        status: String,
    },

    /// Show queue sorted by priority
    Queue,

    /// Show details of a download
    Show { id: String },

    /// Pause a download
    Pause { id: String },

    /// Resume a paused download
    Resume { id: String },

    /// Cancel and remove a download
    Cancel {
        id: String,
        /// Also delete the partial file
        #[arg(long)]
        delete_file: bool,
    },

    /// Show aggregate stats
    Stats,

    /// Probe a URL (HEAD request metadata)
    Inspect { url: String },

    /// Import IDM .ef2 file
    Import {
        file: String,
        /// Add to queue without auto-starting
        #[arg(short = 'q', long)]
        queue_only: bool,
    },

    /// Ensure the daemon is running (starts it if not)
    Daemon,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{} {e:#}", style("error:").red().bold());
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().collect();

    // Check for IDM-style arguments
    let is_idm = args.iter().any(|a| a.eq_ignore_ascii_case("/d"));

    if is_idm {
        let mut new_args = vec![args[0].clone(), "get".to_string()];
        let mut i = 1;
        while i < args.len() {
            match args[i].to_lowercase().as_str() {
                "/d" => {
                    if i + 1 < args.len() {
                        new_args.push(args[i + 1].clone());
                        i += 1;
                    }
                }
                "/p" => {
                    if i + 1 < args.len() {
                        new_args.push("-o".to_string());
                        new_args.push(args[i + 1].clone());
                        i += 1;
                    }
                }
                "/f" => {
                    if i + 1 < args.len() {
                        new_args.push("-f".to_string());
                        new_args.push(args[i + 1].clone());
                        i += 1;
                    }
                }
                "/q" => {
                    new_args.push("--quiet".to_string());
                }
                "/a" => {
                    new_args.push("-q".to_string()); // maps to --queue-only
                }
                _ => {}
            }
            i += 1;
        }
        args = new_args;
    }

    let cli = Cli::parse_from(args);
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    // Auto-start daemon if not reachable
    if !matches!(cli.cmd, Cmd::Daemon) {
        ensure_daemon(&client).await?;
    }

    match cli.cmd {
        Cmd::Get {
            url,
            out,
            filename,
            connections,
            limit,
            watch,
            priority,
            queue_only,
            ytdlp,
            hash,
            quiet,
            auto_extract,
            script,
            schedule,
        } => {
            cmd_get(
                &client,
                url,
                out,
                filename,
                connections,
                limit,
                watch,
                priority,
                queue_only,
                ytdlp,
                hash,
                quiet,
                auto_extract,
                script,
                schedule,
            )
            .await
        }
        Cmd::List { status } => cmd_list(&client, &status).await,
        Cmd::Queue => cmd_queue(&client).await,
        Cmd::Show { id } => cmd_show(&client, &id).await,
        Cmd::Pause { id } => cmd_action(&client, &id, DownloadAction::Pause, "Paused").await,
        Cmd::Resume { id } => cmd_action(&client, &id, DownloadAction::Resume, "Resumed").await,
        Cmd::Cancel { id, .. } => {
            cmd_action(&client, &id, DownloadAction::Cancel, "Cancelled").await
        }
        Cmd::Stats => cmd_stats(&client).await,
        Cmd::Inspect { url } => cmd_inspect(&client, &url).await,
        Cmd::Import { file, queue_only } => cmd_import(&client, &file, queue_only).await,
        Cmd::Daemon => cmd_daemon().await,
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn cmd_get(
    client: &Client,
    url: String,
    out: Option<String>,
    filename: Option<String>,
    connections: u32,
    limit: u64,
    watch: bool,
    priority: String,
    queue_only: bool,
    ytdlp: bool,
    hash: Option<String>,
    quiet: bool,
    auto_extract: bool,
    script: Option<String>,
    schedule: Option<i64>,
) -> Result<()> {
    // Derive priority enum
    let priority_val = match priority.to_lowercase().as_str() {
        "high" | "h" => vajra_protocol::Priority::High,
        "low" | "l" => vajra_protocol::Priority::Low,
        _ => vajra_protocol::Priority::Normal,
    };

    let body = AddDownloadRequest {
        url: url.clone(),
        output_dir: out,
        filename,
        headers: Default::default(),
        expected_hash: hash,
        max_connections: Some(connections),
        speed_limit_bps: if limit > 0 { Some(limit) } else { None },
        priority: priority_val,
        schedule_at: schedule,
        use_ytdlp: ytdlp,
        ytdlp_format: None,
        ytdlp_subtitles: false,
        ytdlp_playlist: false,
        use_http3: false,
        auto_extract,
        post_processing_script: script,
        queue_type: None,
        sync_interval_secs: None,
        tags: Some(vec!["cli".to_string()]),
    };

    let resp = client
        .post(api("/downloads"))
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .context("Daemon rejected the request")?
        .json::<vajra_protocol::AddDownloadResponse>()
        .await?;

    if !quiet {
        println!(
            "{} {} → {}{}",
            style("Added").green().bold(),
            style(&resp.id).cyan(),
            style(resp.filename.as_deref().unwrap_or(&url)).white(),
            if queue_only {
                style(" [queued]").yellow().to_string()
            } else {
                String::new()
            }
        );
    }

    if watch && !quiet {
        watch_progress(client, &resp.id.to_string()).await?;
    }
    Ok(())
}

async fn cmd_queue(client: &Client) -> Result<()> {
    // Show all non-complete downloads sorted by priority (high → normal → low)
    let resp = client
        .get(api("/downloads"))
        .query(&[("status", "all"), ("limit", "100")])
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::DownloadList>()
        .await?;

    let mut items: Vec<_> = resp
        .items
        .iter()
        .filter(|d| {
            !matches!(
                d.status,
                vajra_protocol::DownloadStatus::Completed | vajra_protocol::DownloadStatus::Failed
            )
        })
        .collect();

    // Sort: downloading first, then by priority, then queued/paused
    items.sort_by_key(|d| {
        let pri: u8 = match d.priority {
            vajra_protocol::Priority::High => 0,
            vajra_protocol::Priority::Normal => 1,
            vajra_protocol::Priority::Low => 2,
        };
        let status_ord: u8 = match d.status {
            vajra_protocol::DownloadStatus::Downloading => 0,
            vajra_protocol::DownloadStatus::Idle => 1,
            vajra_protocol::DownloadStatus::Paused => 2,
            _ => 3,
        };
        (status_ord, pri)
    });

    if items.is_empty() {
        println!("{}", style("Queue is empty.").dim());
        return Ok(());
    }

    println!(
        "{}  {:<12} {:<8} {:>10}  {}",
        style("#").bold(),
        style("STATUS").bold(),
        style("PRI").bold(),
        style("DONE").bold(),
        style("FILENAME").bold(),
    );
    println!("{}", style("─".repeat(70)).dim());

    for (i, d) in items.iter().enumerate() {
        let pri_label = match d.priority {
            vajra_protocol::Priority::High => "high",
            vajra_protocol::Priority::Normal => "normal",
            vajra_protocol::Priority::Low => "low",
        };
        let pri_style = match d.priority {
            vajra_protocol::Priority::High => style(pri_label).red().bold(),
            vajra_protocol::Priority::Low => style(pri_label).dim(),
            _ => style(pri_label).yellow(),
        };
        let status_str = d.status.to_string();
        let status_col = match d.status {
            vajra_protocol::DownloadStatus::Downloading => style(status_str).green(),
            vajra_protocol::DownloadStatus::Paused => style(status_str).yellow(),
            vajra_protocol::DownloadStatus::Failed => style(status_str).red(),
            _ => style(status_str).dim(),
        };
        println!(
            "{:>2}  {:<12} {:<8} {:>10}  {}",
            style(i + 1).dim(),
            status_col,
            pri_style,
            fmt_bytes(d.bytes_done),
            style(&d.filename).white(),
        );
    }
    Ok(())
}

async fn cmd_list(client: &Client, status: &str) -> Result<()> {
    let resp = client
        .get(api("/downloads"))
        .query(&[("status", status), ("limit", "100")])
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::DownloadList>()
        .await?;

    if resp.items.is_empty() {
        println!("{}", style("No downloads.").dim());
        return Ok(());
    }

    println!(
        "{:<38} {:<12} {:>10} {:>10} {:>6}  {}",
        style("ID").bold(),
        style("STATUS").bold(),
        style("DONE").bold(),
        style("SPEED").bold(),
        style("PROG%").bold(),
        style("FILENAME").bold()
    );
    println!("{}", style("─".repeat(100)).dim());

    for d in &resp.items {
        let status_str = d.status.to_string();
        let status_color = match d.status {
            vajra_protocol::DownloadStatus::Downloading => style(status_str).green(),
            vajra_protocol::DownloadStatus::Completed => style(status_str).cyan(),
            vajra_protocol::DownloadStatus::Failed => style(status_str).red(),
            vajra_protocol::DownloadStatus::Paused => style(status_str).yellow(),
            _ => style(status_str).dim(),
        };
        println!(
            "{:<38} {:<12} {:>10} {:>10} {:>6.1}  {}",
            style(d.id.to_string()).dim(),
            status_color,
            fmt_bytes(d.bytes_done),
            fmt_speed(d.speed_bps),
            d.progress_pct,
            style(&d.filename).white()
        );
    }
    println!("{}", style(format!("\n{} item(s)", resp.total)).dim());
    Ok(())
}

async fn cmd_show(client: &Client, id: &str) -> Result<()> {
    let d = client
        .get(api(&format!("/downloads/{id}")))
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::DownloadInfo>()
        .await?;

    let progress_pct = d.progress_pct;

    println!("{}: {}", style("ID").bold(), style(d.id).cyan());
    println!("{}: {}", style("URL").bold(), d.url);
    println!("{}: {}", style("File").bold(), d.filename);
    println!("{}: {}", style("Status").bold(), d.status);
    println!(
        "{}: {} / {}",
        style("Progress").bold(),
        fmt_bytes(d.bytes_done),
        d.total_bytes.map(fmt_bytes).unwrap_or_else(|| "?".into())
    );
    println!(
        "{}: {} ({:.1}%)",
        style("Speed").bold(),
        fmt_speed(d.speed_bps),
        progress_pct
    );
    if let Some(e) = &d.error {
        println!("{}: {}", style("Error").red().bold(), e);
    }
    Ok(())
}

async fn cmd_action(client: &Client, id: &str, action: DownloadAction, verb: &str) -> Result<()> {
    let body = PatchDownloadRequest {
        action: Some(action),
        speed_limit_bps: None,
        max_connections: None,
        priority: None,
        url: None,
        tags: None,
        queue_type: None,
        sync_interval_secs: None,
        filename: None,
    };
    client
        .patch(api(&format!("/downloads/{id}")))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    println!("{} {}", style(verb).green().bold(), style(id).cyan());
    Ok(())
}

async fn cmd_stats(client: &Client) -> Result<()> {
    let s = client
        .get(api("/stats"))
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::StatsResponse>()
        .await?;

    println!("{}", style("Vajra Daemon Stats").bold().underlined());
    println!("  Active:    {}", style(s.active_count).green());
    println!("  Queued:    {}", s.queued_count);
    println!("  Paused:    {}", style(s.paused_count).yellow());
    println!(
        "  Speed:     {}",
        style(fmt_speed(s.aggregate_speed_bps)).cyan()
    );
    println!("  Uptime:    {}s", s.daemon_uptime_seconds);
    Ok(())
}

async fn cmd_inspect(client: &Client, url: &str) -> Result<()> {
    let body = vajra_protocol::InspectRequest {
        url: url.to_string(),
        headers: Default::default(),
    };
    let r = client
        .post(api("/inspect"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::InspectResponse>()
        .await?;

    println!("{}: {}", style("URL").bold(), r.effective_url);
    println!(
        "{}: {}",
        style("File").bold(),
        r.filename.unwrap_or_else(|| "unknown".into())
    );
    println!(
        "{}: {}",
        style("Type").bold(),
        r.content_type.unwrap_or_else(|| "?".into())
    );
    println!(
        "{}: {}",
        style("Size").bold(),
        r.total_bytes
            .map(fmt_bytes)
            .unwrap_or_else(|| "unknown".into())
    );
    println!(
        "{}: {}",
        style("Ranges").bold(),
        if r.accepts_ranges {
            style("yes").green()
        } else {
            style("no").red()
        }
    );
    println!(
        "{}: {}",
        style("yt-dlp").bold(),
        if r.ytdlp_supported {
            "supported"
        } else {
            "not needed"
        }
    );
    Ok(())
}

async fn cmd_import(client: &Client, file: &str, queue_only: bool) -> Result<()> {
    let content =
        std::fs::read_to_string(file).with_context(|| format!("Failed to read {}", file))?;

    let body = vajra_protocol::ImportEf2Request {
        content,
        paused: queue_only,
    };

    let resp = client
        .post(api("/import/ef2"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<vajra_protocol::ImportEf2Response>()
        .await?;

    println!(
        "{}",
        console::style(format!(
            "Successfully imported {} downloads.",
            resp.imported_count
        ))
        .green()
        .bold()
    );

    for err in resp.errors {
        println!("{}", console::style(format!("- Error: {}", err)).red());
    }

    Ok(())
}

async fn cmd_daemon() -> Result<()> {
    // Try connecting first
    let client = Client::builder().timeout(Duration::from_secs(2)).build()?;
    if client.get(api("/")).send().await.is_ok() {
        println!("{}", style("Daemon already running.").green());
        return Ok(());
    }
    start_daemon()?;
    println!("{}", style("Daemon started.").green());
    Ok(())
}

// ─── Progress watcher ─────────────────────────────────────────────────────────

async fn watch_progress(client: &Client, id: &str) -> Result<()> {
    let bar = ProgressBar::new(100);
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos:>3}% | {msg}")?
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );

    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let d = match client
            .get(api(&format!("/downloads/{id}")))
            .send()
            .await?
            .error_for_status()
        {
            Ok(r) => r.json::<vajra_protocol::DownloadInfo>().await?,
            Err(_) => break,
        };

        let progress_pct = d.progress_pct;

        bar.set_position(progress_pct as u64);
        bar.set_message(format!(
            "{} | {}",
            fmt_speed(d.speed_bps),
            d.eta_seconds
                .map(|s| format!("ETA {s}s"))
                .unwrap_or_default()
        ));

        match d.status {
            vajra_protocol::DownloadStatus::Completed => {
                bar.finish_with_message(style("Complete ✓").green().to_string());
                break;
            }
            vajra_protocol::DownloadStatus::Failed => {
                bar.finish_with_message(
                    style(format!("Failed: {}", d.error.unwrap_or_default()))
                        .red()
                        .to_string(),
                );
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

// ─── Daemon auto-start ────────────────────────────────────────────────────────

async fn ensure_daemon(client: &Client) -> Result<()> {
    let health = client
        .get(format!("http://127.0.0.1:{}/health", DEFAULT_PORT))
        .timeout(Duration::from_millis(500))
        .send()
        .await;
    if health.is_ok() {
        return Ok(());
    }
    // Try to start
    start_daemon()?;
    // Wait up to 3s
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if client
            .get(format!("http://127.0.0.1:{}/health", DEFAULT_PORT))
            .timeout(Duration::from_millis(200))
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }
    }
    anyhow::bail!(
        "Could not connect to vajrad on port {}. Start it manually with `vajra daemon`.",
        DEFAULT_PORT
    )
}

fn start_daemon() -> Result<()> {
    let exe = std::env::current_exe()?;
    let daemon = exe.with_file_name(if cfg!(windows) {
        "vajrad.exe"
    } else {
        "vajrad"
    });
    if !daemon.exists() {
        anyhow::bail!("vajrad not found at {:?}", daemon);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(&daemon)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(&daemon).spawn()?;
    }
    Ok(())
}

// ─── Formatting helpers ───────────────────────────────────────────────────────

fn fmt_bytes(b: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

fn fmt_speed(bps: u64) -> String {
    format!("{}/s", fmt_bytes(bps))
}

// ─── Clap styles ──────────────────────────────────────────────────────────────

fn clap_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::White.on_default())
}
