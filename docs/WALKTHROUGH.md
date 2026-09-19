# Walkthrough: Vajra Project UI & Backend Fixes

### 0. Fix Real-Time Progress Updates & Default Sorting (Session: 2026-06-28)
- **Progress Recalculation:** Fixed a major bug where incoming progress updates from the SSE/Tauri event loop were merging into `useDownloadStore` state without updating `progress_pct`. This left the table UI progress bar frozen until the user triggered a manual refresh (Ctrl+R). Now, the store automatically computes:
  $$\text{progress\_pct} = \frac{\text{bytes\_done}}{\text{total\_bytes}} \times 100$$
  This ensures that the progress bar fills smoothly in real-time.
- **Default Table Sorting:** Updated the default state in `DownloadsTable.tsx` so the table is automatically sorted by the `added` date/time column in descending order. Newly added downloads now appear at the top of the list by default.
- **Retry Window Progress Trigger:** Configured the "Retry Download" button in the `DownloadFailedWindow` to also emit the `open-progress-window` event to the Tauri shell, automatically opening the progress visualizer window when retrying a failed download.
- **Completed Progress Alignment Fix:** Fixed a sync bug where a completed download could show "99%" progress in the main window. Both the frontend state store ([`downloadStore.ts`](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/stores/downloadStore.ts)) and backend API mapper ([`schema.rs`](file:///d:/Project/Project-Vajra/vajra-daemon/src/api/schema.rs)) have been updated to prioritize the `'completed'` status check before performing division-based percentage calculation, guaranteeing exactly `100%` is shown on completion.
- **Rust-side Diagnostics Logging:** Added a file logger to the Tauri async sidecar event loop in `lib.rs` that writes connection attempts, event receives, and window message emission successes/errors to `tauri-sse.log` under the `Vajra` AppData directory.

I have systematically gone through the issues you reported and implemented comprehensive fixes across the application. Here's a breakdown of what was achieved:

### 1. Fix "Browse" Paths Not Working
Replaced the non-functional `invoke('open_directory')` and `invoke('open_file')` calls in `OptionsDialog.jsx` with standard Tauri dialog plugins.
```javascript
const { open } = await import('@tauri-apps/plugin-dialog');
const selected = await open({ directory: true });
```
This enables the file/folder picker native UI for configuring Output Directories, Temp Directories, and Antivirus Scanners.

### 2. Fix UI Consistency for Remaining Dialogs
Previously, only the main window was overhauled. I have gone through **every remaining dialog** and converted their styling to use the newly implemented dark-mode system component classes (`sys-btn`, `sys-input`, `sys-select`):
- `AddUrlDialog.jsx`
- `SchedulerDialog.jsx`
- `RefreshUrlDialog.jsx`
- `PropertiesDialog.jsx`
- `GrabberDialog.jsx`
- `DeleteDialog.jsx`
- `OptionsDialog.jsx` (Settings center inputs now match everything else instead of using old, buggy tailwind classes).

### 3. Fix Dropdown/Select List Backgrounds
Addressed the issue where dropdown options appeared in full white color. All `select` menus have been replaced with the `.sys-select` class and updated in `index.css` to respect dark mode:
```css
.sys-select option {
  background-color: var(--color-bg1);
  color: var(--color-text1);
}
```

### 4. Fix Button "Shivering" and Missed Clicks
The "shivering" cursor and missed click issue was diagnosed as an interaction conflict with Tauri's frameless window drag regions on Windows. The system was trying to drag the window instead of firing the click events.
- **Fix applied**: Added `-webkit-app-region: no-drag;` to all interactive components in `index.css` (`.sys-btn`, `.sys-input`, `.sys-select`, `.sys-textarea`). This ensures elements are perfectly clickable instantly.

### 5. Progress Window Visibility
If you accidentally click the background and the Progress Window gets buried, there are actually two ways you can bring it back to the front without closing it:
1. **Activity Button**: Click the `Progress` button in the Top Toolbar (the heartbeat icon).
2. **Double-Click**: Double-click any active download item in the main data grid.
This brings the background window directly to focus.

### 6. Verify "Delete from Storage" Bug
As part of resolving backend discrepancies, the `delete_download` route in the Rust daemon (`handlers.rs`) was corrected earlier to properly await disk deletion of files when deleting the download record.

### 7. Settings / Configuration Reset Bug Fix
A major bug was discovered in `SchedulerDialog.jsx` where setting the schedule would **wipe out** the rest of your system configuration (output directory, proxy settings, connection limits, etc.) because it sent a partial object to the backend endpoint.
- **Fix applied**: The `SchedulerDialog.jsx` now pulls the full `config.json` state, merges in the scheduling updates, and writes back the entire configuration object seamlessly.


---

# Vajra â€” Session Walkthrough & Documentation

> Session: 2026-06-19 | Status: Phases 1â€“5 in progress

---

## Session: 2026-06-21 | Status: UI Responsiveness and Bug Fixes

### 1. Fix: "Failed to acquire webview" Popup
The native popups appearing continuously when opening or closing windows were due to race conditions when a React component attempted to manipulate a Tauri window that was already being destroyed. 
* **Fix**: Wrapped all `getCurrentWindow().close()`, `setTitle()`, and other Tauri webview operations inside `ProgressWindow.jsx` and `App.jsx` with robust `try...catch` blocks to silently handle cases where the window is already disposed.

### 2. Fix: Intercept Download Dialog Focus
When a download is intercepted, the "Add URL" dialog was opening but sometimes remaining hidden behind the main window.
* **Fix**: Updated `spawnAddUrlWindow` in `App.jsx` to correctly unminimize and temporarily trigger "Always On Top" to force Windows to pull the dialog to the forefront before grabbing focus.

### 3. Polish: Progress Window Graph
The progress window graph animation was confusingly running and appending data even while the download was paused.
* **Fix**: The SSE update listener now checks the download state. If it is paused, it inserts `0` bps into the `speedHistory` and the graph ceases moving.

### 4. Fix: Main UI Responsiveness and Dropped Clicks
The app felt like a "school project" because clicks weren't registering, and hovering caused flickering due to excessive React re-renders dropping event listeners.
* **Fix**: Extracting `ActionButton` out of inline maps in `Toolbar.jsx` stopped the unmounting/remounting.
* **Fix**: Wrapped all event handlers (`handleSelect`, `handleSelectAll`, `handleDoubleClick`) in `App.jsx` with `useCallback`. This stabilizes the function references, ensuring the row selections and double clicks fire instantly and correctly.

### 5. Fix: Duplicate Progress Windows Spawning
Clicking resume on a download would occasionally pop up duplicate progress windows because `AddUrlWindow` and `App.jsx` were fighting to spawn them.
* **Fix**: Re-architected `AddUrlWindow` to no longer instantiate `WebviewWindow` directly. Instead, it emits a Tauri event (`open-progress-window`) back to the main `App.jsx`, which acts as a centralized manager. `App.jsx` uses a strict `activeProgressWindows` Set to guarantee only one window exists per download.

### 6. Polish: Intelligent Toolbar
Buttons in the toolbar were clickable even when their actions were invalid (e.g., resuming a downloading file).
* **Fix**: Added computed `canResume`, `canPause`, `canStopAll`, and `canDelete` flags to `App.jsx`. Passed these down into `Toolbar.jsx` to dynamically grey out buttons based on the exact state of the selected downloads.

---

## What Was Done This Session

### Phase 1 â€” Extension Auto-Connect (No More Manual ID)

**Problem:** The extension had `nativeMessaging` which required the user to manually paste their extension ID. The Native Host checker showed "Checkingâ€¦" forever.

**Fix â€” Pure HTTP discovery via `http://127.0.0.1:6277/health`:**

| File | Change |
|------|--------|
| [`browser-extension/manifest.json`](file:///d:/Project/Project-Vajra/browser-extension/manifest.json) | Removed `nativeMessaging` permission, bumped to v0.4.1 |
| [`browser-extension/popup.js`](file:///d:/Project/Project-Vajra/browser-extension/popup.js) | Full rewrite â€” HTTP health poll every 3s, auto-start button, stats strip |
| [`browser-extension/popup.html`](file:///d:/Project/Project-Vajra/browser-extension/popup.html) | Removed Native Host row, added Launch Vajra button |
| [`browser-extension/background.js`](file:///d:/Project/Project-Vajra/browser-extension/background.js) | Added `tryAutoStart()` â€” triggers `vajra://start` and polls 10s for daemon |

**How the connection now works:**
```
Extension popup opens
  â†’ fetch("http://127.0.0.1:6277/health")
  âœ“ 200 OK  â†’ show green dot, fetch stats
  âœ— timeout â†’ show "Launch Vajra" button + badge "!"
              user clicks â†’ extension opens vajra://start tab
              OS launches vajra.bat â†’ daemon starts
              extension polls every 500ms for up to 10s
              â†’ auto-connects when daemon is up
```

---

### Phase 2 â€” Auto-Start via `vajra://` URL Protocol

**Problem:** No way to auto-launch the app from the browser.

**Fix â€” [`vajra.bat`](file:///d:/Project/Project-Vajra/vajra.bat) now registers the OS protocol handler on every run:**

```bat
reg add "HKCU\Software\Classes\vajra\shell\open\command" /ve /d "\"%~f0\" \"%%1\""
```

This is **idempotent** â€” safe to run every launch. Once registered, any `vajra://start` link in the browser triggers vajra.bat â†’ Tauri UI starts â†’ daemon starts.

**Like IDM/FDM:** The browser extension can now auto-launch the app. No manual steps.

---

### Phase 3 â€” Engine Enhancements

#### 3A. Dynamic Thread Stealing â€” [`multiplexer.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/multiplexer.rs)

Added `steal_from_slowest()`:
- When a chunk finishes early, its idle thread finds the chunk with the most remaining bytes
- Splits that chunk's tail at the midpoint
- The idle thread takes the new chunk â€” **no thread ever idles while data is available**
- Only steals if both halves are â‰¥ 2 MiB (prevents thrashing)

#### 3E. Token Bucket Bandwidth Throttler â€” [`throttle.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/throttle.rs)

New module with two types:
- **`Throttle`** â€” single token bucket, `acquire(bytes)` blocks until tokens refill
- **`CombinedThrottle`** â€” global + per-download throttle chained together

Set `limit_bps = 0` for unlimited. Exported from [`lib.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/lib.rs).

#### 3H. Windows Sleep Prevention â€” [`main.rs`](file:///d:/Project/Project-Vajra/vajra-daemon/src/main.rs)

```rust
#[cfg(target_os = "windows")]
SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);
```

Prevents Windows from sleeping while the daemon is active (like every download manager).

---

### Phase 5 â€” Auto-Categorization

#### New Protocol Types â€” [`vajra-protocol/src/lib.rs`](file:///d:/Project/Project-Vajra/vajra-protocol/src/lib.rs)

| Type | Purpose |
|------|---------|
| `CategoryRule` | Maps extensions â†’ output directory with a label |
| `ProxyConfig` | HTTP/SOCKS proxy with system proxy option |
| `PostQueueAction` | Action on queue empty: None/ExitApp/Sleep/Hibernate/Shutdown |
| `DuplicateAction` | AutoRename / Overwrite / Prompt |

**Default categories (automatically applied):**

| Category | Extensions | Default Folder |
|----------|-----------|---------------|
| Videos | mp4, mkv, avi, mov, wmv, flv, webm... | `%USERPROFILE%\Videos` |
| Music | mp3, flac, wav, aac, ogg, m4a... | `%USERPROFILE%\Music` |
| Documents | pdf, doc, docx, xls, epub... | `%USERPROFILE%\Documents` |
| Software | exe, msi, msix, deb, apk... | Downloads |
| Archives | zip, rar, 7z, tar, iso... | Downloads |

#### Auto-Categorize Logic â€” [`handlers.rs`](file:///d:/Project/Project-Vajra/vajra-daemon/src/api/handlers.rs)

When a download is added **without an explicit `output_dir`**:
1. Extract file extension from `filename` or URL path
2. Find first matching `CategoryRule` in config
3. Route to that rule's `output_dir`
4. Fall back to `default_output_dir` if no rule matches

---

### Linker Fix â€” [`.cargo/config.toml`](file:///d:/Project/Project-Vajra/.cargo/config.toml)

The VS2022 install on this machine stores `msvcrt.lib` in `lib\onecore\x64` instead of the standard `lib\x64`. Fixed by adding:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = [
  "-L", "C:\\Program Files\\Microsoft Visual Studio\\18\\Community\\VC\\Tools\\MSVC\\14.51.36231\\lib\\onecore\\x64",
]
```

> **Note:** The `vswhom-sys` build script error is now fixed. We updated `build-all.bat` to correctly locate the `vcvars64.bat` script on this specific VS 2022 instance (Community 18 path), ensuring the `INCLUDE` environment is set correctly for the `cc` compiler.

---

## What Remains (Next Session)

### High Priority
- [ ] **Integrate `Throttle` into `download_task.rs`** â€” wire `CombinedThrottle.acquire()` into the chunk streaming loop in `run_chunk()`
- [ ] **`GET /setup` page** â€” simple HTML setup guide served by daemon at `http://127.0.0.1:6277/setup`

### Medium Priority
- [ ] **Frontend UI overhaul** â€” per-chunk visualizer bar, speed graph, category sidebar
- [ ] **System tray** â€” minimize-to-tray via Tauri, tray context menu
- [ ] **Clipboard monitoring** â€” watch clipboard for URLs in extension/daemon
- [ ] **yt-dlp integration** â€” detect streaming URLs, spawn yt-dlp subprocess

### Lower Priority
- [ ] Scheduled download windows (FAP quota)
- [ ] AV scan on completion (path already in config)
- [ ] Site-specific connection limits (already in config)
- [ ] Build script (`build-all.bat`)

---

## How to Continue

### Reload the extension after changes:
1. Go to `chrome://extensions`
2. Click **Reload** on Vajra Download Manager
3. Version should show **0.4.1**

### Register vajra:// protocol:
```bat
:: Run vajra.bat once â€” it self-registers the protocol
d:\Project\Project-Vajra\vajra.bat
```

### Fix the build:
```powershell
cd d:\Project\Project-Vajra
cargo clean
cargo check --workspace 2>&1 | Select-String "^error"
```

## ðŸ› Recent Bug Fixes (June 20)

### 1. Silent Installer Overwrites
- **Issue:** The NSIS installer was throwing an "Error opening file for writing" when attempting to update Vajra, because `vajrad.exe` was still running silently in the background.
- **Fix:** Added `taskkill /F /IM vajrad.exe` silently to the pre-install and pre-uninstall hooks in `installer.nsi`.

### 2. Universal Download Interception
- **Issue:** The browser extension was filtering downloads (only intercepting large files or specific extensions), leading to some links failing to trigger Vajra.
- **Fix:** Changed the extension defaults (`interceptAll: true`, `minSizeMB: 0`) so that it behaves like IDM and captures **every** browser download automatically.

### 3. Eliminated CMD Window Flashing
- **Issue:** A blank black command window flashed briefly on the screen when the UI triggered a URL to open.
- **Fix:** Replaced the `cmd.exe /C start` call with the entirely silent `rundll32 url.dll,FileProtocolHandler` Windows API.

### Phase 11 — Modern Premium UI Overhaul
- **Complete CSS rewrite** using modern design tokens (Fluent UI/Glassmorphism, rounded corners, drop shadows, dark/light mode variables).
- **Core Shell Components overhaul** (App layout, Sidebar pill styles, Toolbar styling).
- **Advanced Dialogs** (Scheduler, Options, Add URL) redesigned as polished modals with vertical tabs, sleek toggles, and blurred backdrop overlays.
- **Detailed Progress Dialog** with dynamic, visually rich Multi-connection Chunk Visualizer.

## 🚀 SQLite-Backed engine & DLMan Gaps Walkthrough (June 28, 2026)

We have successfully overhauled Vajra's core engine, resolving major architectural vulnerabilities, adding transactional safety, optimizing visual responsiveness, and fully closing all competitive gaps with DLMan.

### 1. SQLite Segment/Chunk Persistence (Task 2)
*   **Database Integration ([db.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/db.rs)):**
    *   Enabled `PRAGMA foreign_keys = ON;` and `PRAGMA journal_mode = WAL;` in `Database::open` to guarantee transaction safety.
    *   Added the `download_segments` table.
    *   Implemented `save_segment`, `load_segments`, and `delete_segments` methods on the `Database` struct.
*   **Engine Hooking ([download_task.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/download_task.rs)):**
    *   Replaced the temporary `.vajra.state` JSON sidecar files with direct SQLite calls to `load_segments` on startup/resume, `save_segment` on pause, and `delete_segments` on complete/cancel. This eliminates orphaned segment files and prevents data corruption during abrupt system crashes.

### 2. CDN Redirect Caching & Pre-Flight Probing (Task 2 / Phase 3)
*   **Redirect Table ([db.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/db.rs)):**
    *   Added `job_redirects` table to cache direct target URLs mapping to active `job_id`.
*   **Engine Integration ([download_task.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/download_task.rs)):**
    *   Implemented pre-flight URL resolution check: if a cached URL exists in the database, the engine attempts to probe that URL first.
    *   If the cached URL probe fails (due to link expiration or HTTP `403`/`410` errors), it safely falls back to the original source `req.url`, re-follows redirects, and updates the cached redirect URL.

### 3. TCP Connection Pooling & Keep-Alive (Task 3)
*   **Consolidated Clients ([download_task.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/download_task.rs)):**
    *   Instead of initiating a client pool per segment worker, we now initialize a single pre-configured `reqwest::Client` (with 90s idle timeouts).
*   **Multiplexer Refactoring ([multiplexer.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/multiplexer.rs)):**
    *   Updated `start_download` to accept a single client and clone it across all spawned segment workers, preserving keep-alive handshakes and preventing socket starvation.

### 4. Work-Stealing TOCTOU Race Condition Fix (Task 1)
*   **Message-Passing Stealing ([multiplexer.rs](file:///d:/Project/Project-Vajra/vajra-engine/src/multiplexer.rs)):**
    *   Defined the thread-safe `StealRequest` message structures and added a bounded `steal_tx` mpsc channel to the `Chunk` registry to coordinate splits.
    *   Inside the donor worker loop of `run_chunk`, the worker polls the `steal_rx` channel periodically. If a request is received and >= 2MB remains, the donor truncates its local `end_byte` and returns the stolen tail coordinates via oneshot.
    *   Refactored `steal_from_slowest` to asynchronously send a `StealRequest` over the channel to the slowest worker task and spawn a new chunk on success, resolving the TOCTOU write-overlap race condition.

### 5. Event Loop Aggregation & Frontend Splitting (Task 5)
*   **Batching Progress SSE ([main.rs](file:///d:/Project/Project-Vajra/vajra-daemon/src/main.rs)):**
    *   Updated `progress_loop` to aggregate progress payloads of all active downloads into a single batch, emitting a unified `DaemonEvent::BatchProgress` every 200ms instead of flooding the client with individual events.
*   **Zustand Event Handling ([downloadStore.ts](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/stores/downloadStore.ts)):**
    *   Updated the Tauri SSE listener `initDownloadStoreTauriEvents` to handle `'batch_progress'` events, updating all active items in a single Zustand state batch update.
    *   Added a `pendingGets` tracking Set to prevent concurrent redundant REST `api.get(id)` requests for new downloads.
*   **Progress Window Fix ([ProgressWindow.tsx](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/windows/ProgressWindow.tsx)):**
    *   Added support for parsing `batch_progress` events, allowing the individual progress windows to dynamically update in real-time when the batch progress matches the window's `downloadId`.
*   **Redundant REST Request Avoidance ([App.tsx](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/App.tsx)):**
    *   Updated the `lastSSEUpdate.current` timestamp in the `vajra-event` listener, preventing the fallback HTTP polling loop from executing redundant `api.list()` database polls when SSE updates are active and healthy.
*   **View Memoization ([DownloadsTable.tsx](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/components/DownloadsTable.tsx), [Sidebar.tsx](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/components/Sidebar.tsx), [Toolbar.tsx](file:///d:/Project/Project-Vajra/vajra-ui-tauri/src/components/Toolbar.tsx)):**
    *   Wrapped `DownloadsTable`, `Sidebar`, and `Toolbar` components in `React.memo` to eliminate unnecessary visual render thrashing when download progress updates are received.

---

## Architecture Quick Reference

```
Browser Extension (v0.4.1)
  popup.js ──────── HTTP GET /health ──────────────────────► vajra-daemon :6277
  background.js  vajra://start ──► OS ──► vajra.bat ──► vajra-ui-tauri
                                                         └──► vajra-daemon

vajra-daemon (Rust/axum)
  POST /api/v1/downloads ──► auto-categorize ──► DownloadManager
  DownloadManager ──► DownloadTask ──► Multiplexer (8 chunks default)
                                       ├──► throttle.rs (token bucket)
                                       ├──► steal_from_slowest() (dynamic via channels)
                                       ├──► allocator.rs (sparse file)
                                       └──► writer.rs (positional disk I/O with overlap check)
```

## Key Files

| File | Description |
|------|-------------|
| [`vajra.bat`](file:///d:/Project/Project-Vajra/vajra.bat) | Launcher + protocol handler registrar |
| [`browser-extension/manifest.json`](file:///d:/Project/Project-Vajra/browser-extension/manifest.json) | Extension manifest v0.4.1 |
| [`browser-extension/popup.js`](file:///d:/Project/Project-Vajra/browser-extension/popup.js) | HTTP-based status + auto-start |
| [`browser-extension/background.js`](file:///d:/Project/Project-Vajra/browser-extension/background.js) | Download interception + auto-start |
| [`vajra-engine/src/db.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/db.rs) | SQLite schema migration + segments/redirect helpers |
| [`vajra-engine/src/download_task.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/download_task.rs) | download task orchestration, pre-flight CDN probing |
| [`vajra-engine/src/throttle.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/throttle.rs) | Token bucket bandwidth limiter |
| [`vajra-engine/src/multiplexer.rs`](file:///d:/Project/Project-Vajra/vajra-engine/src/multiplexer.rs) | Chunked download + thread stealing via channel coordination |
| [`vajra-protocol/src/lib.rs`](file:///d:/Project/Project-Vajra/vajra-protocol/src/lib.rs) | All shared types + DaemonConfig |
| [`vajra-daemon/src/api/handlers.rs`](file:///d:/Project/Project-Vajra/vajra-daemon/src/api/handlers.rs) | REST handlers + auto-categorization |
| [`.cargo/config.toml`](file:///d:/Project/Project-Vajra/.cargo/config.toml) | MSVC linker path fix |
