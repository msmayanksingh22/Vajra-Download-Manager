# Vajra — Full Scale Implementation Plan

> **Master Roadmap & Project Plan for Vajra Download Manager**

---

## Current State (What Works ✓)
- **Tauri UI**: Modern Glassmorphism UI (fluent design tokens) with comprehensive windows for settings, scheduler, adding downloads, properties, and a fully functional data grid. Supports real-time progress updates (with dynamic `progress_pct` calculation) and default sorting by download added date (newest first).
- **Daemon (`vajrad`)**: Core Rust engine running on port 6277. Features a highly concurrent HTTP download engine, dynamic segment stealing, sparse file allocation, ETag resume validation, RAM-buffered I/O flushing, token bucket bandwidth throttling, and SQLite task persistence. Includes a global event SSE stream and webhooks.
- **Browser Extension (`v1.0.0`)**: Automatically intercepts downloads and detects media streams (`.m3u8`, `.mpd`, `.mp4`). Communicates cleanly with the Vajra daemon via HTTP. Shows floating "Download" buttons on detected streams.
- **Media Engine**: Native `yt-dlp` integration for handling complex video sites and DASH streams.
- **Build System**: Windows `.exe` + `.msi` installers via NSIS, integrating protocol handlers (`vajra://`) and auto-startup logic.

---

## Roadmap Phases

### [✓] Phase 1 & 2: Browser Auto-Connect & Auto-Start
- Dropped complex Native Messaging for a simpler HTTP heartbeat polling system in the extension.
- Added `vajra://` custom URL protocol on Windows.
- Extension triggers `vajra://start` to boot the daemon automatically if a user clicks a download link while the daemon is offline.

### [✓] Phase 3: Core Engine Enhancements
- **Dynamic Thread Stealing:** Reallocates segments from slow threads to fast threads dynamically.
- **Sparse File Allocation:** Prevents NTFS disk fragmentation.
- **RAM-Buffered I/O:** 16MB ring buffers per chunk, flushing to disk periodically to save SSD wear.
- **Resume Validation:** Strict ETag / Last-Modified checks.
- **Token Bucket Throttling:** Fine-grained speed limits.
- **Header Spoofing & Redirects:** Full support for User-Agent, Referer, and Cookies bypassing hotlink protections.

### [✓] Phase 4: Modern Premium UI Overhaul
- Segment progress visualizer (`ChunkBar`).
- Advanced settings panel (Storage, Network, Antivirus, Proxy).
- Queue manager with drag-and-drop.
- UI Overhaul across all secondary windows (Progress, Add URL, Properties, Scheduler) matching the deep-dark glassmorphism theme.

### [✓] Phase 5: Auto-Categorization, Intelligence & Polish
- Rule-based routing of downloads into specific output folders based on file extension.
- Anomaly detection for downloads.
- Mobile companion app (Scaffold).
- Automation rules engine.
- Plugin/extension system (WebAssembly/Extism).

### [✓] Phase 6 & 13: Browser Extension & Advanced Core Features
- [x] `yt-dlp.exe` bundled integration.
- [x] Browser extension background network listener for media streams.
- [x] Floating `⚡ Grab Stream` injected via Content Scripts.
- [x] HTTP/3 (QUIC) Support.
- [x] ML-Based Operations (Filename cleanup, connection count prediction).
- [x] Content Processing Pipeline (image, video, PDF) / WASM Plugins.
- [x] Collaboration Features (Shared queues, audit logs).
- [x] Captcha Solving & Link Decryption (DLC, RSDF).
- [x] A/B Testing Framework.

### [x] Phase 7: Site Spider / Grabber
- [x] `vajra-daemon/src/spider.rs` implemented.
- [x] Batch download a website's links by depth.
- [x] Filter by regex and extensions.
- [x] Dedicated UI tree view for batch selection.

### [x] Phase 9: CLI Enhancements
- Expanded headless arguments for `vajra-cli` (add, pause, resume, list, remove).
- Headless daemon management.

### [x] Phase 14: Automation & Convenience
- **[x] Clipboard Monitor:** System-wide URL detection and toast notification.
- **[x] Auto-Extraction:** Automatic unpacking of `.zip` / `.rar` / `.7z`.
- **[x] Post-Processing:** Custom bash/batch script execution post-download.
- **[x] Antivirus Scan:** Automatically run Windows Defender or custom AV on complete.

### [x] Phase 15: BitTorrent & Protocol Expansion
- **[x] BitTorrent Support:** Native `.torrent` + magnet link support via `librqbit`. (Done)
- **[x] FTP Support:** Core FTP protocol implementation.

### [x] Phase 16: Cross-Platform & Polish
- [x] macOS (`.dmg`) and Linux (`.AppImage`) Tauri pipelines.
- [x] Multi-language support (i18n).
- [x] Extension multi-select batch downloading.
- [x] **Site Logins Vault:** Centralized credential manager automatically injecting HTTP Basic Auth for matching domains.

### [x] Phase 17: Browser Extension Revamp & Expansion
- **[x] Modern UI Redesign:** Completely overhaul the extension's popup UI to match Vajra's dark glassmorphism aesthetic.
- **[x] Deep Browser Integration:** Add context menu entries (Right-click -> "Download with Vajra").
- **[x] Advanced Media Detection:** Improved overlay on streaming videos.
- **[x] Unified Settings:** Sync extension settings with the daemon (theme, dark mode, connection status).
- **[x] Download Interception:** Fully block browser native save dialog via `onDeterminingFilename` dummy injection.

### [✓] Phase 7: UI/UX Audit & Systematic Refactor (v1.4.1)
- Comprehensive UI/UX audit producing `UI_UX_SUPREME_PLAN.md` covering all components and dialogs.
- **Phase 1 — Style scrub:** Eliminated all hardcoded hex colors and magic-number pixels; entire UI driven by CSS custom properties.
- **Phase 2 — Navigation chrome:** `MenuBar`, `Sidebar`, `Toolbar` refactored to use design tokens; replaced all legacy `window.*` dialogs.
- **Phase 3 — Downloads table:** Rich empty state with CTA, robust `resume_supported` column logic, `<AlertCircle>` error icons, Reset Columns feature.
- **Phase 4 — Dialogs:** `useDialogEscape` shared hook; full `role="dialog"` / `aria-modal` / `aria-labelledby` accessibility pass across all 10 dialogs; live version in `AboutDialog`; dual-action `DeleteDialog`; auto-save `Saved ✓` indicator in `PropertiesDialog`.
- **Phase 5 — Window Chrome:** Standardized `dialog-header` / `dialog-body` / `dialog-footer` structure across all 10 dialogs. Consistent `btn-icon` + `<X>` close button. Remaining native browser alert calls eliminated.
- **Phase 6 — Dashboard Analytics:** Full `Dashboard.tsx` rewrite — KPI cards (Active, Completed, Failed, Total Bytes), Recharts speed history sparkline, recent activity feed. `onNavigate` CTA wiring in `App.tsx`.
- **Phase 7 — Accessibility:** `useFocusTrap` hook applied to all 10 dialogs. `aria-sort` on all table headers. `aria-label` + `aria-disabled` on toolbar buttons. Sidebar converted to `<nav>` with `aria-current="page"` and keyboard navigation.
- **Hotfix:** Toolbar `role="group"` divs given `display: flex` via CSS attribute selector — fixes vertical button stack regression.
