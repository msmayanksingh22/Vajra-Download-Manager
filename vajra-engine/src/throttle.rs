//! Token Bucket Bandwidth Throttler
//!
//! Provides per-download and global speed limiting.
//! Each segment calls `acquire(bytes)` before writing, which blocks
//! (async sleep) until enough tokens are available.
//!
//! The bucket refills at `limit_bps` bytes per second.
//! Setting limit to 0 means unlimited (acquire returns immediately).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

// ─── Token Bucket ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Bucket {
    /// Bytes per second limit. 0 = unlimited.
    limit_bps: u64,
    /// Current available tokens (bytes).
    tokens: f64,
    /// Max tokens = 1 second worth of data (burst cap).
    capacity: f64,
    /// Last refill timestamp.
    last_refill: Instant,
}

impl Bucket {
    fn new(limit_bps: u64) -> Self {
        let cap = if limit_bps == 0 {
            f64::MAX
        } else {
            limit_bps as f64
        };
        Self {
            limit_bps,
            tokens: cap,
            capacity: cap,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        if self.limit_bps == 0 {
            return;
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.limit_bps as f64).min(self.capacity);
    }

    /// Returns how many milliseconds to wait before `bytes` tokens are available.
    fn wait_ms_for(&self, bytes: u64) -> u64 {
        if self.limit_bps == 0 || self.tokens >= bytes as f64 {
            return 0;
        }
        let deficit = bytes as f64 - self.tokens;
        let wait_secs = deficit / self.limit_bps as f64;
        (wait_secs * 1000.0).ceil() as u64
    }

    /// Consume tokens (call after waiting).
    fn consume(&mut self, bytes: u64) {
        if self.limit_bps == 0 {
            return;
        }
        self.tokens = (self.tokens - bytes as f64).max(0.0);
    }

    fn set_limit(&mut self, limit_bps: u64) {
        self.limit_bps = limit_bps;
        let cap = if limit_bps == 0 {
            f64::MAX
        } else {
            limit_bps as f64
        };
        self.capacity = cap;
        if self.tokens > cap {
            self.tokens = cap;
        }
    }
}

// ─── Public Throttle Handle ───────────────────────────────────────────────────

/// A shared, async-safe token bucket throttle.
///
/// Clone to share between multiple segment tasks.
/// Set `limit_bps = 0` for unlimited speed.
#[derive(Clone, Debug)]
pub struct Throttle(Arc<Mutex<Bucket>>);

impl Throttle {
    /// Create a new throttle. `limit_bps = 0` means unlimited.
    pub fn new(limit_bps: u64) -> Self {
        Self(Arc::new(Mutex::new(Bucket::new(limit_bps))))
    }

    /// Unlimited throttle (no delay).
    pub fn unlimited() -> Self {
        Self::new(0)
    }

    /// Acquire permission to send `bytes` bytes.
    /// Sleeps the appropriate amount if we'd exceed the limit.
    pub async fn acquire(&self, bytes: u64) {
        loop {
            let wait_ms = {
                let mut bucket = self.0.lock().await;
                bucket.refill();
                let ms = bucket.wait_ms_for(bytes);
                if ms == 0 {
                    bucket.consume(bytes);
                }
                ms
            };
            if wait_ms == 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        }
    }

    /// Update the speed limit at runtime (0 = unlimited).
    pub async fn set_limit(&self, limit_bps: u64) {
        let mut bucket = self.0.lock().await;
        bucket.set_limit(limit_bps);
    }

    /// Current limit in bytes/sec. 0 = unlimited.
    pub async fn limit_bps(&self) -> u64 {
        self.0.lock().await.limit_bps
    }
}

// ─── Global + Per-Download Throttle ──────────────────────────────────────────

/// Combines a global throttle (shared across all downloads) with a
/// per-download throttle. Both must have tokens available.
#[derive(Clone, Debug)]
pub struct CombinedThrottle {
    pub global: Throttle,
    pub local: Throttle,
}

impl CombinedThrottle {
    pub fn new(global: Throttle, local_limit_bps: u64) -> Self {
        Self {
            global,
            local: Throttle::new(local_limit_bps),
        }
    }

    /// Acquire tokens from both global and local buckets.
    pub async fn acquire(&self, bytes: u64) {
        // Acquire local first (smaller limit typically gates first)
        self.local.acquire(bytes).await;
        self.global.acquire(bytes).await;
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[tokio::test]
    async fn unlimited_returns_immediately() {
        let t = Throttle::unlimited();
        let start = Instant::now();
        t.acquire(1024 * 1024).await; // 1 MB
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn limited_throttle_delays() {
        // 100 KB/s limit, request 50 KB — should be immediate (within burst)
        let t = Throttle::new(100 * 1024);
        let start = Instant::now();
        t.acquire(50 * 1024).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn set_limit_updates_rate() {
        let t = Throttle::new(1024);
        t.set_limit(0).await; // unlimited
        assert_eq!(t.limit_bps().await, 0);
    }
}
