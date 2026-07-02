use std::{collections::VecDeque, sync::Arc};

use tokio::sync::RwLock;

pub struct SpeedTracker {
    history: RwLock<VecDeque<u64>>,
    max_samples: usize,
}

impl SpeedTracker {
    pub fn new(max_samples: usize) -> Arc<Self> {
        Arc::new(Self {
            history: RwLock::new(VecDeque::with_capacity(max_samples)),
            max_samples,
        })
    }

    pub async fn add_sample(&self, speed: u64) {
        let mut hist = self.history.write().await;
        if hist.len() >= self.max_samples {
            hist.pop_front();
        }
        hist.push_back(speed);
    }

    pub async fn get_history(&self) -> Vec<u64> {
        self.history.read().await.iter().copied().collect()
    }
}
