## Context

Currently, the `vajrad` background daemon broadcasts state updates and browser download interceptions via an SSE endpoint. The frontend (React running in the Tauri Main window) connects to this SSE stream. If the Main window is hidden (minimized to system tray), Chromium's background tab throttling often pauses JavaScript execution, causing the SSE connection to drop or lag. This results in missing the `intercepted` events from the browser extension, and the `AddDownload` window never appears. Additionally, having multiple windows (Progress, Main) manage their own SSE connections or rely on the Main window for state updates creates lag and synchronization issues.

## Goals / Non-Goals

**Goals:**
- Shift the SSE connection from the Chromium WebView to the Tauri Rust backend.
- Have the Tauri Rust backend broadcast daemon events natively using Tauri v2's event system (`tauri::Emitter::emit` and `emit_to`).
- Ensure all WebviewWindows (Main, Progress, AddDownload) receive synchronized state updates instantly without individual SSE connections.
- Ensure the `AddDownload` window triggers reliably even when the Main window is hidden or throttled.

**Non-Goals:**
- Changing the `vajrad` daemon's architecture or its SSE endpoint implementation.
- Refactoring the entire React frontend to remove Zustand (we only replace the transport layer of Zustand updates).

## Decisions

1. **Rust-Level SSE Client**:
   We will implement a `reqwest_eventsource` (or similar async stream) client in `src-tauri/src/lib.rs` (or a dedicated `events.rs` module). This Rust task will connect to `http://127.0.0.1:6277/api/v1/sse` upon startup. Because Rust is not subject to Chromium's background throttling, it will reliably receive all events.

2. **Native Event Relaying**:
   The Rust task will receive SSE events and relay them using `app_handle.emit("vajra-event", payload)`. This broadcasts the event to all active Tauri windows natively, ensuring absolute synchronization across the Progress window and Main window.

3. **Window Spawning from Rust (or Un-throttled JS)**:
   For `vajra-intercepted` events, the Rust backend can either:
   a) Directly spawn the `addUrl` WebviewWindow using Tauri's `WebviewWindowBuilder`, guaranteeing it appears immediately.
   b) Emit to the main window and rely on it to spawn.
   **Decision**: We will maintain window creation in the frontend `App.tsx` for now, but rely on Tauri's native IPC which wakes the WebView efficiently. If throttling still prevents execution, we will move the `WebviewWindowBuilder` invocation for `addUrl` directly into Rust. *We will try the Tauri event approach first, as `tauri::Emitter` triggers listeners reliably.*

4. **Frontend Store Refactoring**:
   In `downloadStore.ts`, we will remove `connectSSE` and replace it with `import { listen } from '@tauri-apps/api/event'`. The `listen("vajra-event")` callback will process updates exactly as the old SSE callback did, but with zero lag.

## Risks / Trade-offs

- **Risk**: Rust SSE client might drop connection if the daemon restarts.
  **Mitigation**: Implement a reconnect loop with exponential backoff in the Rust async task.
- **Risk**: Duplicate events if the old JS SSE client isn't fully removed.
  **Mitigation**: Entirely remove `connectSSE` from `api.ts` and `downloadStore.ts` to ensure only the native Tauri listener is active.
