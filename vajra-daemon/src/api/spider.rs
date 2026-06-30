use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use axum::{
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use url::Url;

use crate::DaemonError;

#[derive(Deserialize)]
pub struct SpiderParams {
    url: String,
    depth: Option<u32>,
    regex: Option<String>,
    extensions: Option<String>,
}

#[derive(Serialize)]
pub struct SpiderResult {
    pub url: String,
    pub resource_type: String, // "video", "audio", "image", "document", "page", "other"
    pub name: String,
}

pub async fn run_spider(
    Query(params): Query<SpiderParams>,
) -> std::result::Result<
    Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>,
    DaemonError,
> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let target_url = params.url.clone();
    let max_depth = params.depth.unwrap_or(1);
    let regex_pattern = params.regex.clone();
    let ext_filter = params.extensions.clone();

    tokio::spawn(async move {
        let _ = spider_task(target_url, max_depth, regex_pattern, ext_filter, tx).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|res: SpiderResult| {
        let json = serde_json::to_string(&res).unwrap_or_default();
        Ok(Event::default().data(json))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(5))))
}

async fn spider_task(
    start_url: String,
    max_depth: u32,
    regex_pattern: Option<String>,
    ext_filter: Option<String>,
    tx: tokio::sync::mpsc::Sender<SpiderResult>,
) -> anyhow::Result<()> {
    // ── Security caps ─────────────────────────────────────────────────────────
    // Cap depth regardless of the caller-supplied value to prevent exponential
    // fetch explosion on large sites.
    let max_depth = max_depth.min(3);
    // Hard page-count limit prevents runaway spiders on deeply nested sites.
    const MAX_PAGES: usize = 500;
    // Overall spider timeout: abandoned SSE connections must not hold resources.
    const SPIDER_TIMEOUT_SECS: u64 = 300; // 5 minutes

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;

    // ── Domain scoping ────────────────────────────────────────────────────────
    // Restrict crawling to the same registered domain (eTLD+1) as the start URL.
    // This prevents the spider from being directed at internal hosts or unrelated
    // third-party origins via crafted `<a href>` tags in the fetched HTML.
    let start_parsed = Url::parse(&start_url)?;
    let allowed_host = start_parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Start URL has no host"))?
        .to_lowercase();

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    queue.push_back((start_url.clone(), 0));

    let link_selector = scraper::Selector::parse("a, img, video, source, iframe").unwrap();
    let compiled_regex = regex_pattern.and_then(|r| regex::Regex::new(&r).ok());
    let allowed_exts: HashSet<String> = ext_filter
        .map(|s| {
            s.split(',')
                .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                .filter(|e| !e.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(SPIDER_TIMEOUT_SECS);
    let mut pages_fetched: usize = 0;

    while let Some((url_str, depth)) = queue.pop_front() {
        // Enforce the overall timeout and page cap.
        if tokio::time::Instant::now() >= deadline || pages_fetched >= MAX_PAGES {
            break;
        }

        if !visited.insert(url_str.clone()) {
            continue;
        }

        let base_url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(_) => continue,
        };

        // ── SSRF guard: block private / loopback IPs ─────────────────────────
        // Resolve the host and reject RFC-1918, link-local, and loopback addresses.
        if let Some(host) = base_url.host_str() {
            if is_private_host(host) {
                continue;
            }
        }

        // ── Domain scope guard ────────────────────────────────────────────────
        let base_host = base_url.host_str().unwrap_or("").to_lowercase();
        // Allow the exact host and direct subdomains of the start host.
        if base_host != allowed_host && !base_host.ends_with(&format!(".{}", allowed_host)) {
            continue;
        }

        let html = match client.get(&url_str).send().await {
            Ok(resp) => {
                if let Ok(text) = resp.text().await {
                    text
                } else {
                    continue;
                }
            }
            Err(_) => continue,
        };

        pages_fetched += 1;

        let extracted = {
            let document = scraper::Html::parse_document(&html);
            let mut ext = HashSet::new();

            for element in document.select(&link_selector) {
                if let Some(href) = element.value().attr("href") {
                    ext.insert(href.to_string());
                }
                if let Some(src) = element.value().attr("src") {
                    ext.insert(src.to_string());
                }
                if let Some(data_src) = element.value().attr("data-src") {
                    ext.insert(data_src.to_string());
                }
            }
            ext
        };

        for link in extracted {
            if link.starts_with("javascript:")
                || link.starts_with("mailto:")
                || link.starts_with("data:")
            {
                continue;
            }

            if let Ok(resolved) = base_url.join(&link) {
                let resolved_str = resolved.to_string();
                if visited.contains(&resolved_str) {
                    continue;
                }

                // Domain-scope check on resolved URL before queuing.
                let resolved_host = resolved.host_str().unwrap_or("").to_lowercase();
                if resolved_host != allowed_host
                    && !resolved_host.ends_with(&format!(".{}", allowed_host))
                {
                    // Emit non-page cross-origin resources (e.g. CDN images) but
                    // do NOT recurse into them.
                    let path = resolved.path().to_lowercase();
                    let resource_type = classify_url(&path);
                    if resource_type != "page" {
                        let name = resolved
                            .path_segments()
                            .and_then(|mut s| s.next_back().filter(|n| !n.is_empty()))
                            .map(str::to_string)
                            .unwrap_or_else(|| resolved_host.clone());
                        let should_emit = if !allowed_exts.is_empty() {
                            let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                            allowed_exts.contains(&ext)
                        } else if let Some(re) = &compiled_regex {
                            re.is_match(&resolved_str)
                        } else {
                            true
                        };
                        if should_emit {
                            let _ = tx
                                .send(SpiderResult {
                                    url: resolved_str,
                                    resource_type: resource_type.to_string(),
                                    name,
                                })
                                .await;
                        }
                    }
                    continue;
                }

                let path = resolved.path().to_lowercase();
                // Use the last non-empty path segment as the display name.
                let name = resolved
                    .path_segments()
                    .and_then(|mut s| s.next_back().filter(|n| !n.is_empty()))
                    .map(str::to_string)
                    .unwrap_or_else(|| "page.html".to_string());
                let resource_type = classify_url(&path);

                let should_emit = if !allowed_exts.is_empty() {
                    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                    allowed_exts.contains(&ext)
                } else if let Some(re) = &compiled_regex {
                    re.is_match(&resolved_str)
                } else {
                    true
                };

                if should_emit {
                    let _ = tx
                        .send(SpiderResult {
                            url: resolved_str.clone(),
                            resource_type: resource_type.to_string(),
                            name,
                        })
                        .await;
                }

                if resource_type == "page" && depth < max_depth {
                    queue.push_back((resolved_str, depth + 1));
                } else {
                    visited.insert(resolved_str);
                }
            }
        }
    }

    Ok(())
}

/// Returns `true` if `host` resolves to a private, loopback, or link-local
/// address that must not be fetched by the spider (SSRF prevention).
///
/// This check operates on the hostname/IP string as provided by the URL parser.
/// It covers the most common SSRF targets; a full defence would require an
/// async DNS resolution step, which is left for a future hardening pass.
fn is_private_host(host: &str) -> bool {
    use std::net::IpAddr;

    // Block well-known SSRF targets by name.
    let lower = host.to_lowercase();
    if lower == "localhost"
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower == "metadata.google.internal"
    {
        return true;
    }

    // If it parses as an IP, check for private/loopback/link-local ranges.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback()        // 127.0.0.0/8
                    || v4.is_private()  // 10/8, 172.16/12, 192.168/16
                    || v4.is_link_local() // 169.254/16
                    || v4.is_broadcast()
                    || v4.is_unspecified() // 0.0.0.0
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()       // ::1
                    || v6.is_unspecified() // ::
            }
        };
    }

    false
}

fn classify_url(path: &str) -> &'static str {
    if path.ends_with(".mp4")
        || path.ends_with(".mkv")
        || path.ends_with(".webm")
        || path.ends_with(".avi")
        || path.ends_with(".m3u8")
    {
        "video"
    } else if path.ends_with(".mp3")
        || path.ends_with(".wav")
        || path.ends_with(".flac")
        || path.ends_with(".ogg")
    {
        "audio"
    } else if path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".png")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
    {
        "image"
    } else if path.ends_with(".pdf")
        || path.ends_with(".zip")
        || path.ends_with(".rar")
        || path.ends_with(".7z")
        || path.ends_with(".exe")
    {
        "document"
    } else if path.ends_with(".html") || path.ends_with(".php") || !path.contains('.') {
        "page"
    } else {
        "other"
    }
}
