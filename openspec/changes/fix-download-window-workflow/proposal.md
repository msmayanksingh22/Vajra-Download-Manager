## Why

Currently, clicking a download on a website fails to reliably trigger the `AddDownload` (AddUrl) window because the event flow depends on the Main React window being active and maintaining an SSE connection to the daemon. If the Main window is closed to the tray or asleep, the intercept event is dropped. We need a robust, unified event broadcasting architecture using Tauri v2 to ensure real-time synchronization between the daemon, the AddDownload window, the Progress window, and the Main app.

## What Changes

- **Identify Intercept Flow**: Website download interception is currently handled in the daemon at `vajra-daemon/src/api/handlers.rs` (`intercept_url`), which broadcasts an SSE event.
- **Trace Current Flaw**: The Rust daemon emits an SSE `Intercepted` event. Currently, `App.tsx` listens to this SSE and re-emits it to Tauri to open the AddUrl window. This is fragile.
- **Unified Broadcasting Architecture**: The Tauri Rust shell (`src-tauri/src/lib.rs`) will be modified to maintain a single robust SSE connection to the daemon. It will relay all events using Tauri v2's `tauri::Emitter::emit` to broadcast to all active windows natively.
- **Synchronized State**: Frontend stores (`downloadStore.ts`) in all windows (Main, Progress) will be refactored to listen to these native Tauri events instead of managing their own SSE connections. This ensures state changes instantly reflect across both the Progress window and the Main-app window synchronously without lag.

## Capabilities

### New Capabilities
- `unified-tauri-events`: Establish a single SSE listener in the Tauri Rust backend that broadcasts state updates and intercepts to all frontend WebviewWindows via `tauri::Emitter::emit`.

### Modified Capabilities
- None

## Impact

- **Tauri Shell (`src-tauri/src/lib.rs`)**: Will require a new background thread/task to consume the daemon's SSE stream and emit Tauri events.
- **Frontend Stores (`vajra-ui-tauri/src/stores/downloadStore.ts`)**: Will replace `connectSSE` logic with Tauri's `listen()` for native event updates.
- **Frontend Routing (`vajra-ui-tauri/src/App.tsx`)**: Window spawning logic for `addUrl` will trigger reliably regardless of the main window's visibility state.
