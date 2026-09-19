## Context

The Vajra download manager application intercepts web downloads and presents custom UI windows (AddDownload, Progress, DownloadComplete) via Tauri v2.
There are two core bugs currently impacting this workflow:
1. **Metadata Fetch Failures**: When the application attempts to fetch metadata for an intercepted download URL, the Rust backend performs a HEAD request using `reqwest`. Because this request lacks a `User-Agent` header, many web servers respond with a `403 Forbidden` or `404 Not Found` error, causing a false-positive failure in the `AddDownload` window.
2. **UI Synchronization & Spawning Issues**: The React UI fails to reactively update during a download without a manual refresh (Ctrl+R). Additionally, the `DownloadComplete` window fails to spawn when the backend finishes downloading. This is caused by asynchronous edge cases in how Tauri v2's `listen` API is consumed by the frontend and how Zustand state changes are observed.

## Goals / Non-Goals

**Goals:**
- Fix the false-positive HEAD request failures by mimicking a standard browser.
- Ensure `ProgressWindow` dynamically updates its progress without requiring manual reloads by correctly handling the asynchronous `listen` API of Tauri v2.
- Ensure the `DownloadComplete` window reliably spawns when a download finishes.

**Non-Goals:**
- Refactoring the entire Zustand store architecture.
- Replacing `reqwest` or the `tauri` windowing backend.

## Decisions

1. **Rust `reqwest` User-Agent**: We will add a standard Chrome User-Agent string to the `reqwest::Client` builder inside `vajra-daemon/src/api/handlers.rs`. This provides maximum compatibility with CDNs and static file hosts that block headless/unidentified HTTP clients.

2. **Fix Tauri Event Listener in ProgressWindow**: Tauri v2's `listen()` function is asynchronous and returns a Promise containing the `UnlistenFn`. The `ProgressWindow` currently assigns this promise synchronously, leading to race conditions where events are either dropped or re-registered incorrectly due to React's Strict Mode. We will convert this into a proper `useEffect` with an asynchronous setup function that awaits the unlisten function and cleanly executes it on unmount.

3. **Ensure Reliable Window Spawning**: We will review the `App.tsx` Zustand `subscribe` hook that spawns the `DownloadComplete` window to ensure that the `prevState` logic accurately detects the state transition to `'completed'`, or if necessary, directly listen for the `'completed'` event payload emitted via the Tauri event bridge.

## Risks / Trade-offs

- **Risk**: A static User-Agent could still be blocked by some advanced anti-bot protections (e.g. Cloudflare).
  - **Mitigation**: A standard Chrome UA covers 99% of normal file download hosts. We will also allow the request to fall back gracefully if HEAD still fails.
- **Risk**: Improper cleanup of Tauri event listeners can cause memory leaks.
  - **Mitigation**: We will use a boolean `unmounted` flag inside the asynchronous `useEffect` callbacks to prevent executing the `UnlistenFn` before the listener has finished setting up.
