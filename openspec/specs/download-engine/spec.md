# Download Engine

## Summary

The `vajra-engine` is the core library responsible for handling high-speed, multi-threaded downloads. It operates independently of the daemon or UI, utilizing a Tokio-based asynchronous architecture to maximize disk I/O and network throughput.

## Architecture

1. **Multiplexer & Chunking:**
   - Instead of downloading sequentially, Vajra requests the file size via an HTTP `HEAD` request.
   - It calculates chunks using `multiplexer::calculate_chunks(size, max_connections)`. The minimum chunk size is enforced to prevent over-splitting small files.
   - Each chunk is fetched concurrently using `HTTP GET` with `Range` headers.

2. **Cross-Platform Sparse File Allocation:**
   - Before writing begins, `allocator.rs` reserves the exact file space on the disk.
   - **Windows:** Uses `SetEndOfFile` + `SetFileValidData` for instant allocation.
   - **Linux:** Uses `fallocate(2)`.
   - **macOS:** Uses `fcntl(F_PREALLOCATE)` and `ftruncate`.

3. **Dynamic Thread Stealing:**
   - When a thread finishes downloading its chunk, it queries `steal_from_slowest()`.
   - The engine finds the largest active remaining chunk and splits it, re-assigning the idle thread to the tail end of that chunk. This ensures no thread goes idle until the entire file is fully retrieved.

4. **Sequential Disk Writer:**
   - `writer.rs` receives data streams via an `mpsc` channel. It coordinates writing to disk based on calculated offsets, avoiding full-file memory buffering.

5. **Throttling (Token Bucket):**
   - `throttle.rs` enforces global and per-download speed limits. Tasks must `acquire(bytes)` from the bucket before executing a write cycle.

6. **State Persistence & Resume:**
   - The download state (current chunk cursors) is periodically saved. If Vajra is restarted or the download is interrupted, it can resume from the exact byte cursors without redownloading fetched parts.
