## 1. Rust SSE Client Setup

- [x] 1.1 Add `reqwest-eventsource` and `futures-util` dependencies to `vajra-ui-tauri/src-tauri/Cargo.toml` if not already present.
- [x] 1.2 Implement a background async task in `src-tauri/src/lib.rs` that connects to `http://127.0.0.1:6277/api/v1/sse`.
- [x] 1.3 Add reconnection logic with backoff in the background task to handle daemon restarts.

## 2. Tauri Native Event Emitting

- [x] 2.1 Parse incoming SSE payloads in the Rust task.
- [x] 2.2 Use `app_handle.emit("vajra-event", payload)` to broadcast parsed events to all webview windows.
- [ ] 2.3 Verify `vajra-intercepted`, `progress`, `state_change`, and `added` events are correctly broadcasted via Tauri events.

## 3. Frontend Store Refactoring

- [x] 3.1 Update `vajra-ui-tauri/src/stores/downloadStore.ts` to remove `connectSSE`.
- [x] 3.2 Implement `import { listen } from '@tauri-apps/api/event'` in `downloadStore.ts`.
- [x] 3.3 Map `vajra-event` payloads into the Zustand store's `addOrUpdateDownload` and `batchUpdateDownloads` just like the previous SSE stream.

## 4. Window Initialization Updates

- [x] 4.1 Update `vajra-ui-tauri/src/App.tsx` listener to subscribe to the new Rust-relayed `vajra-intercepted` event (or `vajra-event`).
- [x] 4.2 Verify that `spawnAddUrlWindow` creates and focuses the `addUrl` window even when the main window is closed to the tray.
- [x] 4.3 Remove any legacy SSE connection logic from `vajra-ui-tauri/src/api.ts` to prevent double-connections or duplicates.
- [x] 4.4 Test the synchronized state behavior across the Main app, Progress window, and AddDownload window.
