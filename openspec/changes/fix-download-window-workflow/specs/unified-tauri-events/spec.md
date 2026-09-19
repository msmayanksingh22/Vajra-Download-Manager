## ADDED Requirements

### Requirement: Unified Tauri Event Broadcasting
The Tauri Rust shell SHALL maintain a connection to the daemon's SSE endpoint and relay all received events directly to all frontend windows via Tauri v2's native event emitter (`emit`).

#### Scenario: Relay download progress
- **WHEN** the daemon broadcasts a `progress` SSE event
- **THEN** the Tauri Rust shell intercepts this event and natively emits a corresponding `vajra-event-progress` to all webview windows simultaneously.

#### Scenario: Handle intercepted download reliably
- **WHEN** the daemon broadcasts an `intercepted` SSE event
- **THEN** the Tauri Rust shell intercepts this event and natively emits `vajra-intercepted` to all webview windows.
- **AND THEN** the frontend receives this event regardless of whether the Main Window is actively visible.

### Requirement: Frontend Native Event Listeners
Frontend stores (e.g. `downloadStore.ts`) SHALL utilize Tauri's `listen` function to subscribe to relayed daemon events instead of managing raw SSE connections directly from the UI thread.

#### Scenario: Synchronous UI updates
- **WHEN** a state change or progress event occurs
- **THEN** both the Main window and Progress window receive the Tauri event synchronously and instantly update their UI state without lag.
