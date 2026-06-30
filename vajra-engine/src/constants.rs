use std::time::Duration;

pub const MIN_CHUNK_SIZE: u64 = 128 * 1024;
pub const MAX_RETRIES: u32 = 4;
pub const RAM_FLUSH_THRESHOLD_BYTES: usize = 4 * 1024 * 1024;
pub const WRITER_CHANNEL_CAPACITY: usize = 512;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
pub const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
pub const MAX_REDIRECTS: usize = 10;
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(30);
pub const TICK_INTERVAL_MS: u64 = 250;
pub const FLUSH_TICK_MS: u64 = 250;
pub const MAX_SPIDER_DEPTH: u32 = 3;
pub const MAX_SPIDER_PAGES: usize = 500;
pub const SSE_CHANNEL_CAPACITY: usize = 256;
