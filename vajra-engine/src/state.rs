//! Crash-resilient download state persistence.
//!
//! Each active download writes a `.{filename}.vajra.state` sidecar file
//! atomically (write-to-temp + rename) so a crash never corrupts partial state.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkProgress {
    pub chunk_id: usize,
    /// Bytes successfully written for this chunk.
    pub bytes_written: u64,
    /// Inclusive start offset of this chunk.
    #[serde(default)]
    pub start_byte: Option<u64>,
    /// Inclusive end offset of this chunk.
    #[serde(default)]
    pub end_byte: Option<u64>,
}

/// Full state snapshot of a paused or in-progress download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadState {
    pub id: Uuid,
    pub url: String,
    pub total_bytes: u64,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    pub chunks: Vec<ChunkProgress>,
    pub paused_at: DateTime<Utc>,
}

impl DownloadState {
    /// Write state atomically: write to `.tmp`, then rename.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("vajra.tmp");
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Load state from disk. Returns `None` if file doesn't exist.
    pub fn load(path: &Path) -> std::io::Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(path)?;
        let state: Self = serde_json::from_slice(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Some(state))
    }

    /// Returns bytes written for a given chunk id, or 0 if not found.
    pub fn get(&self, chunk_id: usize) -> Option<&ChunkProgress> {
        self.chunks.iter().find(|c| c.chunk_id == chunk_id)
    }
}
