# Vajra — Development Changelog

> All significant changes logged here for continuity across sessions.

---

## [1.4.1] - 2026-06-28 (Phase 7 — Accessibility, Final Polish & Hotfixes)

### Accessibility (Phase 7)
- **`useFocusTrap` hook** (`src/hooks/useFocusTrap.ts`): Lightweight zero-dependency focus trap. On dialog open, moves focus to the first focusable element and cycles Tab/Shift+Tab within the panel. Applied to all 10 dialogs.
- **`aria-sort` on table headers**: `ResizableHeader` in `DownloadsTable.tsx` now sets `aria-sort="ascending"|"descending"|"none"` and `scope="col"` on every `<th>`. Sort click handler moved to the `<th>` element directly; resize handle stops propagation.
- **`aria-label` + `aria-disabled` on toolbar**: `ActionButton` component now exposes both attributes — screen readers announce button name and disabled state correctly.
- **Sidebar navigation semantics**: Root element changed from `<div>` to `<nav aria-label="Application navigation">`. All sidebar items get `role="button"`, `tabIndex={0}`, `aria-current="page"` (when active), and keyboard `Enter`/`Space` activation handler. Delete buttons get explicit `aria-label` strings.
- Confirmed existing: `@media (prefers-reduced-motion: reduce)`, `*:focus-visible` ring, and `aria-live="polite"` on status bar.

### Window Chrome (Phase 5)
- Standardized `dialog-header` / `dialog-body` / `dialog-footer` CSS class structure across all 10 dialogs.
- Consistent `btn-icon` + `<X>` close button on all panels.
- Remaining `window.alert` / `window.confirm` calls eliminated; replaced with inline error state and confirm UI patterns.

### Dashboard Analytics (Phase 6)
- Full rewrite of `Dashboard.tsx`: live KPI cards (Active, Completed, Failed, Total Bytes transferred), Recharts-based speed history sparkline (last 60 samples), and recent activity feed.
- `App.tsx`: `onNavigate` prop added to `<Dashboard />` — CTA buttons route into sidebar categories.

### Bugfix — Toolbar Group Layout
- `[role="toolbar"] > [role="group"] { display: flex; align-items: center; }` added to `@layer components` in `index.css`. The `role="group"` wrapper divs had no display rule and defaulted to `block`, causing all toolbar buttons to stack vertically and overflow the toolbar area. Single CSS rule restores horizontal layout.

---

## [1.4.0] - 2026-06-28 (Phase 7 — UI/UX Audit & Systematic Refactor)

### Design System (Phase 1 — Hardcoded Style Scrub)
- **Inline style elimination:** Removed all hardcoded hex colors and pixel magic-numbers from `App.tsx` and `DownloadsTable.tsx`. All values now reference CSS custom properties (`var(--color-*)`, `var(--sp-*)`, `var(--radius-*)`, etc.).

### Navigation Chrome (Phase 2)
- **`MenuBar.tsx`:** Replaced all `window.alert` / `window.confirm` calls with proper Tauri event dispatching and component-level state.
- **`Sidebar.tsx`:** Unified active indicator, hover states, and spacing using design tokens.
- **`Toolbar.tsx`:** Standardized all button classes; removed ad-hoc inline styles.

### Downloads Table (Phase 3)
- **Rich empty state:** Full redesign with icon, hierarchy, descriptive copy, and "Add Your First Download" CTA button. Wired via `vajra:open-add-url` custom DOM event bridge to `App.tsx → AddUrlDialog`.
- **Resume column logic:** Added `resume_supported: boolean` to `DownloadInfo` type (`types.ts`); column is conditionally rendered only for eligible statuses (`paused`, `error`).
- **Error indicators:** Replaced raw emoji characters with `<AlertCircle>` SVG icon from `lucide-react` for consistent, scalable error display.
- **Reset Columns:** Added "Reset Columns" item to the column-visibility context menu, restoring default column visibility state.

### Dialogs (Phase 4 — Audit & UX Patterns)
- **`useDialogEscape` hook** (`src/hooks/useDialogEscape.ts`): New shared hook. Attaches a `keydown` listener for `Escape` and calls `onClose`. Zero-config, drop-in for any dialog.
- **Accessibility:** Applied `role="dialog"`, `aria-modal="true"`, and `aria-labelledby` to all 10 dialog panels (`AddUrlDialog`, `RefreshUrlDialog`, `SchedulerDialog`, `ImportContainerDialog`, `GrabberDialog`, `SpiderDialog`, `OptionsDialog`, `AboutDialog`, `DeleteDialog`, `PropertiesDialog`).
- **`AboutDialog`:** Replaced static version string with live fetch via Tauri `getVersion()` API. Improved layout with logo container and license info.
- **`DeleteDialog`:** Split the single delete action into two explicit buttons — **Remove from List** (`onConfirm(false, remember)`) and **Delete from Disk** (`onConfirm(true, remember)`). Added "Remember my choice" checkbox.
- **`PropertiesDialog`:** Added animated **Saved ✓** indicator (`CheckCircle2` icon) in the dialog header. Appears for 1.5 s after each successful auto-save debounce write, then fades out.

---

## [1.3.1] - 2026-06-28 (Bug Fixes & Production Launch Hardening)

### Tauri Application backend (`src-tauri`)
- **Sidecar Spawning Capabilities**: Added `"bin/vajrad"` explicitly to allowed shell spawn capability list in `capabilities/main.json` to prevent production permission blockage of the sidecar in installer builds.
- **Tauri Shell Logger**: Integrated generalized `log_to_file` helper in `lib.rs` and added diagnostic logging to `tauri-shell.log` inside the app setup hook, ensuring errors in launching the sidecar daemon are recorded.

### UI & UX Integrations
- **Top Menu Ribbon Action Handlers**: Wired missing callback routes for `batch` (opens Grabber Dialog) and `help` (opens help docs) actions in `MenuBar.tsx` and updated the `App.tsx` parent handler interface.
- **Completed Downloads Removal Fix**: Corrected Zustand subscriber state transition evaluation logic in `App.tsx` (using `prev && prev.status !== 'completed'`) to prevent race condition notifications/windows from opening when clearing completed downloads.

## [1.3.0] - 2026-06-28 (Phase 6 Execution & Advanced Features)

### Core Engine (`vajra-engine`)
- **Memory-Mapped I/O (mmap)**: Introduced zero-copy writing bypassing thread pools with `MmapHandle` mapped pre-allocated files directly into virtual memory.
- **Linux `io_uring`**: Added asynchronous kernel ring-buffer integration compile checks.
- **Stabilized Rolling ETA**: EMA-smoothed historical rolling ETA calculation.

### UI & UX Integrations
- **Dynamic Smart Lists**: Implemented dynamic custom query-based folders in `Sidebar.tsx` and `DownloadsTable.tsx`.
- **Natural Language Parsing**: Added input sentence parsing ("download all pdfs from http...") that redirects to the Site Spider.
- **Right-to-Left (RTL) Layout**: Dynamic direction shift in Options and document wrapper.
- **Tray Menu Controls & Speed Limits**: Enhanced system tray context menu in `lib.rs` with global resume/pause/add actions, and added inline pill-based speed selector controls inside the downloads table right-click context menu.
- **Brand Sizing & Styling**: Polished header brand icon, extension popup, and About dialog modal.

### Bug Fixes & Code Health
- **Type Compatibility**: Added `speed_limit_bps` to the frontend `DownloadInfo` type interface.
- **Vitest Test Selection**: Excluded `tests/e2e/**` from the Vitest test runner.

## [1.2.0] - 2026-06-28 (Real-Time Progress Recalculation & Table Default Sorting)

### UI & State Store
- **Real-Time Progress Fix:** Added automatic recalculation of `progress_pct` inside `batchUpdateDownloads` in `downloadStore.ts` whenever partial progress events are received from the daemon SSE stream, resolving the issue where progress bars remained frozen at their initial state until manual reload.
- **Default Table Sorting:** Updated `DownloadsTable.tsx` to sort by `added` column descending (newest first) by default instead of `filename` ascending.
- **Tauri Rust-Side SSE Logging:** Integrated file logging to `tauri-sse.log` inside the sidecar connection loop in `lib.rs` to debug any potential connection issues or packet loss in the Rust-to-JS event bridge.

## [1.1.0] - 2026-06-24 (Full UI/UX Overhaul & Design System Unification)

### Design System
- **Unified CSS architecture:** Rewrote `index.css` with a `@theme` block exporting ~80 semantic CSS custom properties (`--color-surface`, `--color-brand`, `--color-text-1…4`, `--color-error`, `--color-warning`, `--radius-*`, `--shadow-*`, `--transition-*`, `--font-*`). All colours, typography, and spacing now flow from one source of truth.
- **Tailwind v4 compatibility fix:** Resolved `@apply btn-base` / `@apply input-field` errors — Tailwind v4 cannot `@apply` user-defined component classes. Replaced all shared button/input base styles with direct property declarations inlined into each variant class.
- **`ThemeContext.tsx`:** Wired OS `prefers-color-scheme` listener so the `data-theme` attribute updates in real time; theme preference is persisted to `localStorage`.
- **Component classes defined:** `btn-primary`, `btn-secondary`, `btn-ghost`, `btn-danger`, `btn-icon`, `input-field`, `textarea-field`, `select-field`, `tag-brand`, `tag-status-*`, `form-group`, `form-label`, `dialog-panel`, `dialog-header`, `dialog-body`, `dialog-footer`, `dialog-overlay`, `sidebar-item`, `table-th`, `table-row`, `drag-region`.

### Components Refactored (Phase 2)
- **`Toolbar.tsx`** — semantic icon colours, `btn-icon` everywhere, removed all hardcoded Tailwind palette classes.
- **`Sidebar.tsx`** — `sidebar-item` active/hover states driven by CSS vars.
- **`MenuBar.tsx`** — unified header height and typography.
- **`DownloadsTable.tsx`** — `table-th`, `table-row`, `tag-brand`, semantic status colours.

### Dialogs Refactored (Phase 3 — all 8 dialogs)
- `OptionsDialog`, `AddUrlDialog`, `DeleteDialog`, `SchedulerDialog`, `GrabberDialog`, `SpiderDialog`, `PropertiesDialog`, `RefreshUrlDialog` — all migrated to `dialog-panel` / `dialog-header` / `dialog-body` / `dialog-footer` pattern. Zero hardcoded colours remain.
- **`OptionsDialog`** — 3-pass batch replacement of ~150 legacy class instances; Toggle component already used CSS vars; radio-button dots, proxy section, site credentials, AV scan, post-process, scheduler all clean.

### Windows Refactored (Phase 4)
- **`DownloadCompleteWindow.tsx`** — `InfoCard` helper, CSS variable inline styles, `S{}` token shortcut.
- **`ProgressWindow.tsx`** — complete rewrite; compact title bar + stats grid + speed/threads toggle + segment bar + chart; `getSegmentColors()` returns CSS var strings; dead JSX tail removed via PowerShell truncation.
- **`AddUrlWindow.tsx`** — complete rewrite; URL textarea, inspect banner, save-as/location, speed/hash grid, checkbox toggles, auth fields, post-processing, footer, duplicate modal — all via `card()` helper and `S{}` token map.

### App Shell (Phase 5)
- **`App.tsx`** — OS window-control buttons with `onMouseEnter`/`onMouseLeave` inline CSS var hover, menu bar drag region, toolbar, sidebar+table layout, status bar (speed + connection dot) — all zero legacy classes.
- Final **full-project grep**: **ALL CLEAN** — zero `bg-bg*`, `text-text-*`, `sys-btn`, `sys-input`, `glass-panel`, `text-brand`, `text-status-*` Tailwind classes remain in any `.tsx`/`.ts` file.

### Build & Verification
- `tsc --noEmit` → **0 errors**
- `npx vite build` → **✓ 4.4s** (only pre-existing Tauri dynamic-import advisory, unrelated to this work)

---

## [1.0.0] - 2026-06-24 (Final Feature Parity Completion)


### Features & Documentation
- **Vault/Auth Architecture:** Added centralized credentials vault (`vault_credentials` via SQLite) with dynamic HTTP Basic Auth injection for corresponding domains.
- **Sync Queueing:** Implemented background periodic synchronization tasks in the daemon using `HEAD` requests and ETag/Last-Modified tracking.
- **FTP Support:** Fully integrated FTP download capabilities.
- **UI Modernization:** Fixed Tailwind configurations (`index.css` variables) and Options dialog state-saving endpoints.
- **Specification Sync:** Used `opsx-sync` and `opsx-archive` to merge all delta specs (`ftp-support`, `site-spider`, `automation-and-convenience`, `modernize-main-ui`) into master OpenSpecs and safely archived them.
- **100% Roadmap Completion:** Verified implementation of all 17 roadmap phases from `PLAN.md` with full parity to `IDM-Functional-and-UI-Analysis.md`.

---

## [0.4.5] - 2026-06-24 (Phase 16 Completion)

### Cross-Platform & Polish
- **macOS & Linux Sidecar Build Pipeline:** Implemented `build-sidecar.mjs` script injected into `beforeBuildCommand` to correctly compile the Vajra daemon (`vajrad.exe`) across multiple platforms natively before Tauri bundles it.
- **Tauri Shell Refactor:** Removed manual `std::process::Command` calls in `lib.rs`, replacing them with the official `tauri_plugin_shell` implementation to robustly spawn and kill the daemon as a bundled Sidecar. 
- **Internationalization (i18n):** Integrated `react-i18next` into the UI (`i18n.ts`, `locales/en.json`, `locales/es.json`) and began wrapping UI components (`Sidebar.tsx`, `MenuBar.tsx`) to support dynamic multi-language localization.

### Extension
- **Batch Downloading Checkbox Injection:** When the user holds the `Alt` key, Vajra intercepts links on the page and injects clickable batch-download checkboxes next to them. A floating action button handles batch dispatch back to the daemon using a new `batch_add_download` message route.

---

## [0.4.4] - 2026-06-23 (Browser Extension Rewrite)

### Extension (Vite + TypeScript)
- **Complete Rewrite:** Replaced legacy JavaScript background scripts with a modern Vite + React + TypeScript extension architecture (`vajra-extension`).
- **Zero-Friction Interception:** Adopted the `chrome.downloads.onDeterminingFilename` API hook to provide a dummy filename, successfully bypassing Chrome's native "Save As" dialogue completely when intercepting downloads.
- **Content Scripts:** Restored `content.ts` injection, which detects `<video>` and `<audio>` streams globally and overlays a "⚡ Download" or "⚡ Stream Grab" button dynamically.
- **Advanced Header Parsing:** Re-enabled `webRequest.onHeadersReceived` to intercept `Content-Disposition` headers early, providing 100% accurate filenames for obfuscated downloads before `onCreated` fires.
- **Context Menus & Modifiers:** Re-enabled right-click "Download with Vajra" context menus and Alt (bypass) / Ctrl (force) keyboard modifiers.

### UI & UX Tweaks
- **Native Notifications:** Connected `tauri-plugin-notification` to `ProgressWindow.tsx` to utilize Windows Native Notifications for download completion events.
- **Global Settings Inheritance:** Removed the individual `max_connections` (threads) override from `AddUrlWindow.tsx` to enforce global settings inheritance, keeping the UI cleaner.

---

## [0.4.3] - 2026-06-21 (Session Update)

### UI & UX Bug Fixes
- **UI Responsiveness:** Fixed extreme lag and unresponsiveness (dropped clicks, flickering). Wrapped event handlers in `useCallback` to prevent unnecessary re-rendering of `DownloadsTable`.
- **Toolbar Intelligence:** Dynamically enabled/disabled buttons in `Toolbar.jsx` based on the status of selected downloads (e.g. Resume is greyed out if already downloading).
- **Progress Window Spawn Collision:** Addressed duplicate progress window popups by refactoring `AddUrlWindow` to emit `open-progress-window` to `App.jsx` rather than creating windows itself. 
- **Dialog Pop-up Mitigation:** Fixed the persistent "Failed to acquire webview" native popup by wrapping Tauri's `getCurrentWindow().close()` and `setTitle()` with robust `try...catch` handlers.
- **Intercept Focus:** Intercepted downloads automatically bring the "Add URL" dialog to the front using unminimize and `setAlwaysOnTop` hacks.
- **Graph Polishing:** Fixed progress graph continuously updating/drawing when the download was paused.

---

## [0.4.2] — 2026-06-19 (Session Update)

### UI & Extension
- **Site Grabber (Spider):** Added `Spider.jsx` with tree view, filtering by type, and batch download via SSE stream from the daemon (Phase 7).
- **Scheduler UI:** Added `Scheduler.jsx` with time picker and post-queue action selector (Phase 4D)
- **Clipboard Monitor:** Added Windows-native clipboard polling in Tauri `lib.rs` and floating grab toast in UI (Phase 4F)
- **Extension (Phase 6D):** Enhanced `content.js` to detect streaming media (`m3u8`, `mpd`, youtube, etc.) and inject a ⚡ Stream Grab button that flags `use_ytdlp: true` to the daemon.
- **Queue UI:** Added Drag-and-drop reordering, priority badges, and Clear Completed button (Phase 4C)
- **Add Dialog:** Added speed limit, hash verification, and connections fields (Phase 4E)

### Daemon (Phase 7)
- **Site Spider / Grabber:** Implemented `spider.rs` with `reqwest` and `scraper` crates. Parses HTML to discover media resources up to 1 layer deep, streaming findings to the UI via `GET /api/v1/spider` using Server-Sent Events (SSE).

### CLI (Phase 9)
- Added short flags to `vajra get`: `/d` (out dir), `/p` (priority), `/q` (queue only), `/f` (filename), `--ytdlp`, `--hash`
- Added `vajra queue` subcommand to list non-complete downloads sorted by priority
- Added `priority` field to `DownloadInfo` in `vajra-protocol`

### Installer & Distribution (Phase 10)
- Packaged the application using a customized NSIS installer (`installer.nsi`)
- Added automatic Windows registry hooks to register the `vajra://` protocol handler
- Injected PowerShell `Add-MpPreference` command during installation to automatically add the app folder to Windows Defender exclusions
- Added Windows Startup registry keys for automatic boot launch
- Configured the uninstaller to cleanly remove all registry keys and Defender exclusions

---

## [0.4.1] — 2026-06-19
- **BREAKING:** Removed `nativeMessaging` permission — extension no longer needs manual ID registration
- Extension now discovers daemon via `GET http://127.0.0.1:6277/health` (HTTP-only)
- `popup.js` fully rewritten: live health dot, stats strip, auto-start button
- `background.js`: added `tryAutoStart()` — opens `vajra://start`, polls daemon 10s
- Context menu: auto-launches Vajra if daemon is down, then queues the download

### Launcher
- `vajra.bat`: registers `vajra://` URL protocol handler on every run (idempotent)
  - `HKCU\Software\Classes\vajra\shell\open\command` → vajra.bat
  - Enables browser extension to auto-start Vajra (like IDM/FDM)

### Engine (`vajra-engine`)
- Added `throttle.rs`: token bucket bandwidth limiter
  - `Throttle` — single bucket with `acquire(bytes).await`
  - `CombinedThrottle` — global + per-download chained
- `multiplexer.rs`: added `steal_from_slowest()` for dynamic thread stealing
  - When a chunk finishes, idle thread splits the biggest active chunk's tail
  - Only steals if both halves ≥ 2 MiB
- `lib.rs`: exported `Throttle`, `CombinedThrottle`

### Daemon (`vajra-daemon`)
- `main.rs`: Windows sleep prevention via `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED)`
- `handlers.rs`: auto-categorization in `add_download` — routes by file extension
- `Cargo.toml`: added `windows = {0.58, Win32_System_Power}` (Windows-only dep)

### Protocol (`vajra-protocol`)
- `DaemonConfig` extended with:
  - `category_rules: Vec<CategoryRule>` — extension → folder mapping
  - `proxy: ProxyConfig` — HTTP/SOCKS proxy with system proxy option
  - `post_queue_action: PostQueueAction` — action on queue empty
  - `duplicate_action: DuplicateAction` — auto_rename / overwrite / prompt
  - `av_scan_path`, `av_scan_args` — post-download AV scan
  - `temp_dir`, `fap_quota_bytes_per_hour`, `site_connection_limits`, `blacklist_domains`
- New types: `CategoryRule`, `ProxyConfig`, `PostQueueAction`, `DuplicateAction`
- Default category rules: Videos → Videos folder, Music → Music folder, etc.

### Build Fix
- `.cargo/config.toml`: added `-L onecore\x64` linker path for VS2022 install quirk

### Cleanup
- Removed stale files: `build-phase2.bat`, `build-rust.bat`, `run-daemon.bat`, `run-ui.bat`
- Removed native messaging remnants: `vajra_nm_manifest.json`, `install_nm.reg`, `install_nm_dev.reg`

---

## [0.4.0] — 2026-06-18

### Extension
- Initial HTTP-based health check implementation
- Context menu download interception
- Min file size filter (default 5 MB)
- Notification on download capture

### Engine
- `multiplexer.rs`: byte-range chunked downloading with exponential backoff retry
- `allocator.rs`: cross-platform sparse file allocation (Windows/Linux/macOS)
- `writer.rs`: sequential disk writer from mpsc channel
- `state.rs`: chunk progress persistence for resume
- `download_task.rs`: full lifecycle (queued → fetching_meta → allocating → downloading → complete)
- `queue.rs`: concurrent download manager with configurable max_concurrent

### Daemon
- Axum HTTP server on port 6277
- SSE event stream for real-time progress
- SQLite database for job persistence and recovery
- Download recovery on daemon restart

### UI
- React + Tauri shell
- Basic download list view

---

## [0.3.x] — Earlier

- Initial project scaffolding
- Native messaging approach (abandoned in 0.4.1)
- Basic FFI prototype
