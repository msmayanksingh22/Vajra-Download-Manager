//! A/B Testing Framework
//!
//! Provides a deterministic feature bucketing mechanism based on client UUID
//! to safely roll out UX or engine changes to a subset of clients.

use std::collections::HashMap;
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub name: String,
    pub rollout_percentage: u8, // 0-100
}

pub struct ExperimentManager {
    client_id: String,
    experiments: HashMap<String, Experiment>,
}

impl ExperimentManager {
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            experiments: HashMap::new(),
        }
    }

    pub fn register_experiment(&mut self, name: &str, rollout_percentage: u8) {
        self.experiments.insert(
            name.to_string(),
            Experiment {
                name: name.to_string(),
                rollout_percentage,
            },
        );
    }

    /// Check if this client falls into the treatment bucket for the given experiment.
    pub fn is_enabled(&self, experiment_name: &str) -> bool {
        if let Some(exp) = self.experiments.get(experiment_name) {
            if exp.rollout_percentage == 0 {
                return false;
            }
            if exp.rollout_percentage == 100 {
                return true;
            }

            // Deterministic hash based on experiment name and client ID
            let mut hasher = Sha256::new();
            hasher.update(format!("{}:{}", experiment_name, self.client_id).as_bytes());
            let hash = hasher.finalize();

            // Use the first byte as an index 0-255 mapped to 0-100
            let val = (hash[0] as f32 / 255.0 * 100.0).round() as u8;
            val < exp.rollout_percentage
        } else {
            false
        }
    }
}
