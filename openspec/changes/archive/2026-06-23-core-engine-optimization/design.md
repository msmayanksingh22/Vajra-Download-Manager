## Context

The current `vajra-daemon` download engine relies on static chunking. At the start of a download, the file is divided into `N` equal segments, and each thread is assigned a static byte range. If one thread is slower than the others (due to routing issues, server load balancing, etc.), the overall download is bottlenecked by the slowest thread. Furthermore, each chunk request establishes a new TLS connection, causing massive overhead on high-latency networks. 

## Goals / Non-Goals

**Goals:**
- Implement "in-half" dynamic segmentation: when a fast thread finishes its chunk, it should aggressively "steal" the second half of the chunk from the slowest active thread.
- Implement TCP Connection Pooling (Keep-Alive) across chunk requests to eliminate repetitive TLS handshakes.
- Maintain existing `status` and `progress` event emitting schemas for the UI.

**Non-Goals:**
- BitTorrent or non-HTTP protocol support (this is Phase 15).
- Browser media sniffing (this is Phase 13).

## Decisions

- **Thread Reassignment Logic:** We will maintain a shared state of active segments using an `Arc<Mutex<Vec<Segment>>>` or similar Rust concurrent data structure. When a thread completes, it will lock the segments, find the active segment with the largest remaining bytes, split it in half, and take ownership of the new half.
- **Connection Pooling:** We will instantiate a single shared `reqwest::Client` with `pool_idle_timeout` and `pool_max_idle_per_host` explicitly configured. We will *not* create a new client per thread or chunk.

## Risks / Trade-offs

- **Risk:** Too frequent splitting could lead to excessive HTTP Range request overhead, negating the speed benefits.
  - *Mitigation:* Establish a `MIN_CHUNK_SIZE` (e.g., 1MB or 2MB). Threads will not split a chunk if the remaining bytes are below this threshold.
- **Risk:** Servers might not support HTTP Keep-Alive or may close the connection aggressively.
  - *Mitigation:* The `reqwest` client inherently handles connection drops and will seamlessly reconnect if the pooled connection was killed by the remote server.
