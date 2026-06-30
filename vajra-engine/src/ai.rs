use std::collections::VecDeque;

pub struct AnomalyDetector {
    speed_history: VecDeque<f64>,
    max_history_size: usize,
}

impl AnomalyDetector {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            speed_history: VecDeque::with_capacity(max_history_size),
            max_history_size,
        }
    }

    pub fn record_speed(&mut self, speed_bps: f64) {
        if self.max_history_size == 0 {
            return;
        }
        while self.speed_history.len() >= self.max_history_size {
            self.speed_history.pop_front();
        }
        self.speed_history.push_back(speed_bps);
    }

    /// Detect if the download speed has mysteriously dropped to zero
    /// despite the connection being open.
    pub fn is_stuck(&self) -> bool {
        if self.speed_history.len() < self.max_history_size {
            return false;
        }
        // If the last 5 readings are 0 but previous readings were high
        let recent_zeros = self.speed_history.iter().rev().take(5).all(|&s| s == 0.0);
        let had_good_speed = self.speed_history.iter().rev().skip(5).any(|&s| s > 1000.0);
        recent_zeros && had_good_speed
    }
}

/// Simple ML-inspired heuristic to clean up release scene filenames.
pub fn clean_filename_ml(filename: &str) -> String {
    // Separate stem and extension
    let path = std::path::Path::new(filename);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(filename);

    // Extract duplicate suffix if any (e.g. " (1)" at the end of the stem)
    let mut stem_to_clean = stem.to_string();
    let mut suffix = String::new();
    if let Ok(re) = regex::Regex::new(r"\s*\(\d+\)$") {
        if let Some(m) = re.find(stem) {
            suffix = m.as_str().to_string();
            stem_to_clean = stem[..m.start()].to_string();
        }
    }

    // Replace dots with spaces in the stem only
    let mut cleaned = stem_to_clean.replace(".", " ");
    
    // Regex patterns for common scene tags
    let patterns = [
        r"(?i)(1080p|720p|2160p|4k|8k|x264|x265|h264|h265|HEVC|BluRay|BRRip|HDRip|WEBRip|WEB-DL|HDTV)",
        r"(?i)(\[.*?\]|\(.*?\))", // Remove anything in brackets or parentheses
        r"(?i)(- ?[A-Za-z0-9]+$)" // Remove trailing group names (e.g., -YTS or -RBG)
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }

    // Clean up extra spaces
    if let Ok(re) = regex::Regex::new(r"\s{2,}") {
        cleaned = re.replace_all(&cleaned, " ").to_string();
    }
    
    let cleaned_stem = format!("{}{}", cleaned.trim(), suffix);
    let cleaned_stem = cleaned_stem.trim();
    if extension.is_empty() {
        cleaned_stem.to_string()
    } else {
        format!("{}.{}", cleaned_stem, extension)
    }
}

/// Predict optimal number of concurrent connections based on file size and server ping.
pub fn predict_optimal_connections(file_size_bytes: u64, latency_ms: Option<u64>) -> usize {
    if file_size_bytes == 0 {
        return 1;
    }

    let mut connections = match file_size_bytes {
        0..=1_048_576 => 1, // <= 1MB
        1_048_577..=10_485_760 => 4, // 1MB - 10MB
        10_485_761..=524_288_000 => 8, // 10MB - 500MB
        _ => 16, // > 500MB
    };

    // If server latency is very high, reduce connections to avoid congestion
    if let Some(latency) = latency_ms {
        if latency > 500 {
            connections = (connections / 2).max(1);
        }
    }

    connections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anomaly_detection() {
        let mut detector = AnomalyDetector::new(10);
        
        // Feed some good speeds
        for _ in 0..5 {
            detector.record_speed(5000.0);
        }
        assert!(!detector.is_stuck());
        
        // Feed 5 zeros
        for _ in 0..5 {
            detector.record_speed(0.0);
        }
        
        // It should now be flagged as stuck
        assert!(detector.is_stuck());
    }

    #[test]
    fn test_clean_filename_ml() {
        assert_eq!(clean_filename_ml("Ollama.dmg"), "Ollama.dmg");
        assert_eq!(clean_filename_ml("Ollama (1).dmg"), "Ollama (1).dmg");
        assert_eq!(clean_filename_ml("Ollama (12).dmg"), "Ollama (12).dmg");
        assert_eq!(clean_filename_ml("Movie.Title.2024.1080p.BluRay.x264.mkv"), "Movie Title 2024.mkv");
    }
}
