## 1. Connection Pooling Implementation

- [x] 1.1 Refactor the `vajra-daemon` download client initializer to create a single global `reqwest::Client` instance per download task.
- [x] 1.2 Explicitly configure `pool_idle_timeout` and Keep-Alive settings in the `reqwest::ClientBuilder` for optimal connection reuse.
- [x] 1.3 Pass the shared `Arc<Client>` reference to all worker threads instead of instantiating new clients per thread.

## 2. Dynamic Segmentation Data Structures

- [x] 2.1 Refactor the `Segment` or `Chunk` struct to be managed inside an `Arc<Mutex<Vec<Segment>>>` so threads can read and write to the global chunk state safely.
- [x] 2.2 Add `bytes_remaining` and `current_offset` tracking specifically for the dynamic splitting algorithm.

## 3. The Reallocation Algorithm

- [x] 3.1 Modify the thread completion logic: when a thread finishes a chunk, it locks the `Vec<Segment>`.
- [x] 3.2 Implement `find_largest_remaining_chunk()` to locate the thread with the most bytes left.
- [x] 3.3 Implement the split logic: If the largest chunk has > 1MB remaining, bisect it, update the slow thread's boundary, and assign the new half to the newly freed thread.
- [x] 3.4 Ensure the new thread correctly sends HTTP Range requests for the bisected chunk using the shared `reqwest::Client`.

## 4. Testing & Verification

- [x] 4.1 Write integration tests or manual verification logs to confirm HTTP connections are reused (zero TLS handshakes after the first).
- [x] 4.2 Verify dynamic splitting triggers appropriately and files assemble correctly with matching hashes.
