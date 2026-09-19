## Why

The application currently fails to handle download interception robustly due to two critical bugs:
1. The backend's HEAD request to fetch download metadata lacks a standard User-Agent header. This causes many servers to reject the request, resulting in a false-positive "HEAD request failed" error in the AddDownload window even when the URL is perfectly valid.
2. The UI (both Main and Progress windows) fails to dynamically re-render or reflect state changes in real-time from the backend's `vajra-event` emissions. State changes are only visible upon a manual reload (Ctrl+R). Additionally, when a download completes, the application fails to spawn the `DownloadComplete` window as expected.
Fixing these issues is essential to restore the core download lifecycle workflow and ensure a smooth, reactive user experience.

## What Changes

- Add a standard browser `User-Agent` header to the `reqwest` client used for HEAD/metadata requests in the Rust backend.
- Debug and fix the React/Zustand state synchronization to ensure the frontend reactively updates when `listen('vajra-event', ...)` triggers.
- Ensure the `DownloadComplete` window correctly spawns and focuses upon receiving a "Complete" event payload from the backend.

## Capabilities

### New Capabilities
None

### Modified Capabilities
- `download-window-workflow`: Updating the requirements around HEAD request user-agents and completion window spawning synchronization.

## Impact

- **Rust Backend**: The `reqwest` client builder for metadata fetching.
- **Frontend State**: The `downloadStore.ts` Zustand store and event listeners in `App.tsx` / `ProgressWindow.tsx`.
- **Window Management**: The Tauri window creation commands for the `DownloadComplete` window.
