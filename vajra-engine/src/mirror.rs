use std::{collections::HashMap, time::Instant};

use chrono::{DateTime, Utc};
use reqwest::Client;

use crate::multiplexer::Chunk;

pub struct MirrorSet {
    pub urls: Vec<Mirror>,
    pub health: HashMap<String, MirrorHealth>,
}

pub struct Mirror {
    pub url: String,
    pub priority: u8,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct MirrorHealth {
    pub latency_ms: u64,
    pub supports_ranges: bool,
    pub speed_bps: f64,
    pub last_checked: DateTime<Utc>,
    pub consecutive_failures: u32,
}

pub struct MirrorManager {
    pub mirrors: MirrorSet,
    client: Client,
}

impl MirrorManager {
    pub fn new(urls: Vec<String>, client: Client) -> Self {
        let mirrors = urls
            .into_iter()
            .map(|url| Mirror {
                url,
                priority: 0,
                enabled: true,
            })
            .collect();

        Self {
            mirrors: MirrorSet {
                urls: mirrors,
                health: HashMap::new(),
            },
            client,
        }
    }

    pub async fn probe_all_mirrors(&self) -> Vec<(String, MirrorHealth)> {
        let mut results = Vec::new();

        for mirror in &self.mirrors.urls {
            if !mirror.enabled {
                continue;
            }

            let health = self.probe_mirror(&mirror.url).await;
            results.push((mirror.url.clone(), health));
        }

        results
    }

    async fn probe_mirror(&self, url: &str) -> MirrorHealth {
        let start = Instant::now();

        // Send HEAD request
        let result = self.client.head(url).send().await;

        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(resp) => {
                let supports_ranges = resp
                    .headers()
                    .get("Accept-Ranges")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.contains("bytes"))
                    .unwrap_or(false);

                MirrorHealth {
                    latency_ms: latency,
                    supports_ranges,
                    speed_bps: 0.0, // Will be measured during download
                    last_checked: Utc::now(),
                    consecutive_failures: 0,
                }
            }
            Err(_) => MirrorHealth {
                latency_ms: u64::MAX,
                supports_ranges: false,
                speed_bps: 0.0,
                last_checked: Utc::now(),
                consecutive_failures: 1,
            },
        }
    }

    pub fn rank_mirrors(&self, results: &[(String, MirrorHealth)]) -> Vec<String> {
        let mut ranked = results.to_vec();

        // Sort by: supports_ranges (bool), latency_ms, speed_bps
        ranked.sort_by(|a, b| {
            let a_score = (
                a.1.supports_ranges as u8,
                a.1.speed_bps,
                1.0 / (a.1.latency_ms as f64).max(1.0),
            );
            let b_score = (
                b.1.supports_ranges as u8,
                b.1.speed_bps,
                1.0 / (b.1.latency_ms as f64).max(1.0),
            );
            b_score
                .partial_cmp(&a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranked.into_iter().map(|(url, _)| url).collect()
    }

    pub async fn assign_chunks_to_mirrors(
        &self,
        chunks: &[Chunk],
        ranked_mirrors: &[String],
    ) -> HashMap<usize, String> {
        let mut assignments = HashMap::new();
        if ranked_mirrors.is_empty() {
            return assignments;
        }

        for (i, chunk) in chunks.iter().enumerate() {
            // Round-robin assignment to top 3 mirrors
            let mirror_idx = i % ranked_mirrors.len().min(3);
            assignments.insert(chunk.id, ranked_mirrors[mirror_idx].clone());
        }

        assignments
    }

    pub async fn handle_mirror_failure(
        &mut self,
        mirror_url: &str,
        _chunk_id: usize,
    ) -> Option<String> {
        // Mark mirror as failed
        if let Some(health) = self.mirrors.health.get_mut(mirror_url) {
            health.consecutive_failures += 1;

            if health.consecutive_failures >= 3 {
                if let Some(mirror) = self.mirrors.urls.iter_mut().find(|m| m.url == mirror_url) {
                    mirror.enabled = false;
                }
            }
        }

        // Re-rank remaining mirrors
        let ranked = self.rank_mirrors(&self.probe_all_mirrors().await);

        // Return new mirror for the chunk
        ranked.first().cloned()
    }
}
