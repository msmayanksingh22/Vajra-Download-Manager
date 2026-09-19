## Why

Vajra currently uses static chunking and creates fresh TLS connections for every chunk. To compete with top-tier download managers like IDM on raw speed, we need to optimize the core engine. Dynamic segmentation will aggressively maximize bandwidth by reassigning idle threads, and protocol pipelining will eliminate the extreme overhead of redundant TLS handshakes.

## What Changes

- Implement dynamic "in-half" chunk reallocation algorithm.
- Abandon slow threads and reassign them to unfinished segments of faster threads.
- Implement TCP connection reuse (Keep-Alive) and protocol pipelining to skip TLS handshakes on subsequent chunk requests to the same host.

## Capabilities

### New Capabilities
- `dynamic-segmentation`: Handles dynamic in-half chunk reallocation and thread reassignment.
- `connection-pooling`: Manages TCP connection reuse and TLS session resumption for chunk requests.

### Modified Capabilities
- `download-engine`: Updating the core download execution loop to support dynamic chunking rather than the static ranges defined at start.

## Impact

- **Rust Backend:** Major changes to the `vajra-daemon` download engine, connection handling, and task scheduling modules.
- **Dependencies:** May require new or updated HTTP/TCP pool management libraries (e.g., `reqwest` connection pooling configs).
- **Performance:** Significant increase in CPU utilization and network throughput.
