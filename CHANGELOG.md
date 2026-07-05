# Changelog

All notable changes to Vajra Download Manager are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) conventions.

---

> **A note on version numbers:** Vajra was developed privately before going public.
> During development, internal build numbers tracked individual phases (v0.4.x → v1.4.x).
> When it came time for the first public release, the version was reset to **v0.2.1**
> to reflect that this is still an early beta. The history below shows the full
> internal development journey leading up to this milestone.

---

## [0.2.1] — 2026-07-04 · 🎉 Grand Public Beta Release

This is the first public release of Vajra. Everything below represents the full
development history leading up to this beta milestone.

### Highlights
- **Multi-segment parallel downloading** — up to 10× faster than native browser downloads
- **Connection stealing** — idle threads dynamically reassigned to the slowest segment
- **OS-level file pre-allocation** — zero-fill bypass via `SetEndOfFile`/`fallocate`/`fcntl`
- **Zero-copy memory-mapped I/O** — direct network-to-disk writes bypassing user-space
- **VPN kill switch** — pauses downloads automatically on interface loss
- **Chrome/Edge Manifest V3 extension** — intercepts, sniffs media streams, batch captures
- **Headless daemon mode** — full REST API + Server-Sent Events at `127.0.0.1:6277`
- **CLI client** (`vajra-cli get`) — scriptable, supports priorities, hash verification, ytdlp (fixed sidecar renaming)
- **Tauri v2 desktop app** — frameless React shell, tray controls, deep-link `vajra://`
- **Custom NSIS Windows installer** — auto-registers URL handler, Defender exclusions

---

## [1.4.1] — 2026-06-28 · Accessibility & Final Polish

### Accessibility (Phase 7)
- `useFocusTrap` hook — focus cycling for all 10 dialogs
- `aria-sort` on all table headers, `aria-label`/`aria-disabled` on toolbar buttons
- Sidebar converted to `<nav aria-label="Application navigation">` with full keyboard navigation
- All dialogs: `role="dialog"`, `aria-modal`, `aria-labelledby`

### Window Chrome (Phase 5)
- Standardized `dialog-header`/`dialog-body`/`dialog-footer` across all dialogs
- Eliminated all `window.alert`/`window.confirm` — replaced with inline confirm UI

### Dashboard Analytics (Phase 6)
- Live KPI cards (Active, Completed, Failed, Total Bytes)
- Recharts speed-history sparkline (last 60 samples)
- Recent activity feed with `onNavigate` CTA routing

### Bug Fix — Toolbar Layout
- `[role="toolbar"] > [role="group"]` now `display: flex` — fixes vertical overflow

---

## [1.4.0] — 2026-06-28 · UI/UX Audit & Systematic Refactor

### Design System
- Eliminated all hardcoded hex colors from `App.tsx` and `DownloadsTable.tsx`
- All values reference CSS custom properties

### Navigation
- `MenuBar.tsx` — replaced `window.alert`/`window.confirm` with Tauri events
- `Sidebar.tsx` — unified active indicator and hover states via design tokens

### Downloads Table
- Rich empty state with "Add Your First Download" CTA
- `resume_supported` column conditional rendering
- `AlertCircle` SVG for error indicators
- Reset Columns in column-visibility menu

### Dialogs
- `useDialogEscape` hook — Escape key handler, drop-in for all dialogs
- `AboutDialog` — live version via `getVersion()`, improved layout
- `DeleteDialog` — split into "Remove from List" / "Delete from Disk" with remember checkbox
- `PropertiesDialog` — animated "Saved ✓" indicator after auto-save debounce

---

## [1.3.1] — 2026-06-28 · Production Launch Hardening

- Sidecar spawn permission added to capabilities
- Tauri shell logger to `tauri-shell.log`
- Fixed batch/help action routes in MenuBar
- Fixed Zustand race condition on completed download removal

---

## [1.3.0] — 2026-06-28 · Phase 6 Execution & Advanced Features

### Engine
- Memory-Mapped I/O — zero-copy writing with `MmapHandle`
- Linux `io_uring` compile integration
- EMA-smoothed rolling ETA

### UI
- Dynamic custom query Smart Lists in Sidebar
- Natural language input parsing ("download all pdfs from http...")
- RTL layout support
- Tray menu: global resume/pause/add
- Speed-limit pill selector in right-click context menu

---

## [1.2.0] — 2026-06-28 · Real-Time Progress & Default Sorting

- `batchUpdateDownloads` auto-recalculates `progress_pct` from SSE events
- Default sort: newest-first by `added` column
- Rust-side SSE logging to `tauri-sse.log`

---

## [1.1.0] — 2026-06-24 · Full UI/UX Overhaul & Design System

- Rewrote `index.css` with `@theme` — ~80 semantic CSS custom properties
- Tailwind v4 compatibility — replaced all `@apply` patterns
- `ThemeContext.tsx` — OS `prefers-color-scheme` live sync + `localStorage` persistence
- Component classes: `btn-primary`, `btn-secondary`, `btn-ghost`, `btn-danger`, `btn-icon`,
  `input-field`, `dialog-panel`, `sidebar-item`, `table-th`, `table-row`, `drag-region`
- All 8 dialogs and 3 windows fully migrated; zero hardcoded colours remain
- `tsc --noEmit` → **0 errors** · `vite build` → **✓ 4.4 s**

---

## [1.0.0] — 2026-06-24 · Feature Parity Completion

- Vault/Auth: centralized credentials with HTTP Basic Auth injection
- Sync Queueing: HEAD + ETag/Last-Modified periodic sync
- FTP download support
- 100% roadmap completion verified (17 phases from `PLAN.md`)

---

## [0.4.5] — 2026-06-24 · Cross-Platform Build Pipeline

- `build-sidecar.mjs` — cross-platform daemon compilation before Tauri bundle
- `tauri_plugin_shell` sidecar spawn (replaces manual `std::process::Command`)
- i18n via `react-i18next` — English + Spanish locale stubs
- Extension: Alt-key batch-download checkbox injection + floating action button

---

## [0.4.4] — 2026-06-23 · Browser Extension Rewrite (Vite + TypeScript)

- Complete rewrite — Vite + React + TypeScript
- `chrome.downloads.onDeterminingFilename` — bypass native "Save As" dialog
- `content.ts` — `<video>`/`<audio>` stream detection + download overlay button
- `webRequest.onHeadersReceived` — `Content-Disposition` parsing for accurate filenames
- Right-click context menus, Alt/Ctrl modifier keys
- Native Windows notifications via `tauri-plugin-notification`

---

## [0.4.3] — 2026-06-21 · UI Responsiveness & Bug Fixes

- `useCallback` wrappers on all table event handlers — eliminated lag and dropped clicks
- Dynamic toolbar button states based on selected download status
- Fixed duplicate progress window popups
- Fixed "Failed to acquire webview" popups with try-catch guards

---

## [0.4.2] — 2026-06-19 · Site Grabber, Scheduler & Queue UI

- Site Spider with tree view, type filters, and SSE batch download
- Scheduler with time picker and post-queue action selector
- Clipboard monitor — Windows-native polling + floating toast
- Extension: streaming media detection (`m3u8`, `mpd`, YouTube) + ⚡ Stream Grab
- Queue: drag-and-drop reorder, priority badges, Clear Completed
- Site Spider daemon: `spider.rs` with `reqwest`+`scraper`, streams findings via SSE

---

## [0.4.1] — 2026-06-19 · Extension HTTP Discovery & Engine Improvements

### Engine
- `throttle.rs` — token bucket bandwidth limiter (`Throttle` + `CombinedThrottle`)
- `multiplexer.rs` — `steal_from_slowest()` dynamic thread stealing (≥2 MiB chunks)

### Daemon
- Windows sleep prevention via `SetThreadExecutionState`
- Auto-categorization by file extension in `add_download`

### Protocol
- `DaemonConfig` extended: `category_rules`, `proxy`, `post_queue_action`, `duplicate_action`, AV scan paths

### Extension
- Removed `nativeMessaging` — pure HTTP discovery via `GET /health`
- `tryAutoStart()` — opens `vajra://start`, polls 10 s
- Context menu auto-launches Vajra if daemon is down

---

## [0.4.0] — 2026-06-18 · Initial Public Build

- Axum HTTP server on port 6277
- SSE event stream for real-time progress
- SQLite persistence with recovery on restart
- Byte-range multiplexed downloads with exponential-backoff retry
- Cross-platform sparse file allocation (Windows/Linux/macOS)
- Sequential disk writer from mpsc channel

---

## [0.3.x] and earlier

- Initial project scaffolding
- Native messaging prototype (abandoned in 0.4.1)
- Basic FFI experiments
