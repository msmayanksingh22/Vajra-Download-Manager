# Vajra — Architecture & Development Guide

> This document lives in the repo. Update it as the project evolves.

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Browser (Chrome / Edge)                                        │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  vajra-extension v1.1.0 (Vite/TS)                          │ │
│  │  background.ts ── bypass Save As dialog + intercept ───────► │─┐
│  │  content.ts    ── inject overlay buttons ──────────────────► │ │
│  │  popup.tsx     ── health poll every 3s ────────────────────► │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                    │ HTTP  POST /api/v1/downloads
                    │ HTTP  GET  /health, /api/v1/stats
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│  vajra-daemon  (Rust / axum)  127.0.0.1:6277                    │
│  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌───────────────┐  │
│  │  Router  │  │ Handlers  │  │   SSE    │  │  DB (SQLite)  │  │
│  └──────────┘  └───────────┘  └──────────┘  └───────────────┘  │
│                      │                                          │
│               ┌──────▼──────────────────────────────────┐      │
│               │  vajra-engine                            │      │
│               │  ┌────────────┐  ┌──────────────────┐   │      │
│               │  │  Queue     │  │  DownloadManager │   │      │
│               │  └────────────┘  └──────────────────┘   │      │
│               │  ┌────────────┐  ┌──────────────────┐   │      │
│               │  │ Multiplexer│  │  Throttle (TB)   │   │      │
│               │  └────────────┘  └──────────────────┘   │      │
│               │  ┌────────────┐  ┌──────────────────┐   │      │
│               │  │ Allocator  │  │  Writer / Spider │   │      │
│               │  └────────────┘  └──────────────────┘   │      │
│               │  ┌────────────┐  ┌──────────────────┐   │      │
│               │  │ Sync Queue │  │  Vault Config    │   │      │
│               │  └────────────┘  └──────────────────┘   │      │
│               └──────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────┘
                    │ Tauri IPC
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│  vajra-ui-tauri  (React + Tauri)  localhost:1420 (dev)          │
│  Download list, per-chunk visualizer, settings, speed graph     │
└─────────────────────────────────────────────────────────────────┘
```

---

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `vajra-engine` | Core download library: queue, multiplexer, allocator, writer, throttle |
| `vajra-protocol` | Shared types: API request/response types, DaemonConfig, CategoryRule |
| `vajra-daemon` | Axum HTTP server exposing REST + SSE API on port 6277 |
| `vajra-cli` | CLI tool for scripted downloads |
| `vajra-ui-tauri` | React frontend wrapped in Tauri desktop shell |

---

## Connection Strategy (v0.4+)

**No native messaging. Pure HTTP.**

```
Extension → GET http://127.0.0.1:6277/health
  ✓ 200  → connected, poll stats every 3s
  ✗ fail → show "Launch Vajra" button
            user clicks → open vajra://start tab
            OS → vajra.bat (registered protocol handler)
            vajra.bat → launches vajra-ui-tauri.exe
            daemon starts → extension auto-connects (polls 500ms × 20)
```

The `vajra://` protocol is registered in `HKCU\Software\Classes\vajra` by `vajra.bat` on every launch (idempotent). This works like IDM / FDM auto-start.

---

## Engine: How Downloads Work

1. **HEAD request** — get `Content-Length`, check `Accept-Ranges: bytes`
2. **Sparse file allocation** — `allocator.rs` reserves disk space without zero-filling  
   - Windows: `SetEndOfFile` + `SetFileValidData`  
   - Linux: `fallocate(2)`  
   - macOS: `fcntl(F_PREALLOCATE)` + `ftruncate`
3. **Chunk calculation** — `multiplexer::calculate_chunks(size, max_connections)`  
   - Min chunk size: 1 MiB (never over-splits small files)
4. **Concurrent HTTP GET with Range headers** — one tokio task per chunk
5. **Exponential backoff retry** — 4 attempts, 250ms → 500ms → 1s → 2s
6. **Dynamic thread stealing** — `steal_from_slowest()`: idle threads split the slowest active chunk
7. **Bandwidth throttling** — `throttle.rs` token bucket: `acquire(bytes)` before each write
8. **Memory-Mapped Zero-Copy Write** — `writer.rs` maps the pre-allocated file directly into virtual memory using `MmapHandle`. Threads write their segments directly into mapped memory regions, avoiding traditional syscall overhead. Automatically falls back to standard sequential channel-based disk writing if `mmap` mapping is unsuccessful or not supported.
9. **State persistence** — transactional SQLite database state tracking (`download_segments`) for highly resilient resumes.

---

## Auto-Categorization

Downloads are automatically routed to the right folder based on file extension.

Default rules (in `DaemonConfig::default()`):

| Category | Extensions | Folder |
|----------|-----------|--------|
| Videos | mp4, mkv, avi, mov, wmv, flv, webm, m4v, ts | `%USERPROFILE%\Videos` |
| Music | mp3, flac, wav, aac, ogg, m4a, wma, opus | `%USERPROFILE%\Music` |
| Documents | pdf, doc, docx, xls, epub, ppt... | `%USERPROFILE%\Documents` |
| Software | exe, msi, msix, deb, rpm, apk | Downloads |
| Archives | zip, rar, 7z, tar, gz, iso, img | Downloads |

Rules are fully configurable via `config.json`. If the caller passes an explicit `output_dir`, it overrides categorization.

---

## REST API Quick Reference

Base URL: `http://127.0.0.1:6277`

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Daemon liveness check |
| `GET` | `/api/v1/downloads` | List all downloads |
| `POST` | `/api/v1/downloads` | Add a download |
| `GET` | `/api/v1/downloads/:id` | Get download info |
| `PATCH` | `/api/v1/downloads/:id` | Pause / Resume / Cancel |
| `DELETE` | `/api/v1/downloads/:id` | Remove download |
| `GET` | `/api/v1/stats` | Aggregate speed, active count |
| `POST` | `/api/v1/inspect` | HEAD a URL, return metadata |
| `GET` | `/api/v1/config` | Get current DaemonConfig |
| `PATCH` | `/api/v1/config` | Update config |
| `GET` | `/events` | SSE stream of download events |

### POST /api/v1/downloads body
```json
{
  "url": "https://example.com/file.mp4",
  "filename": "optional-override.mp4",
  "output_dir": null,
  "max_connections": 8,
  "speed_limit_bps": 0,
  "priority": "Normal",
  "use_ytdlp": false,
  "headers": {
    "Cookie": "session=abc",
    "Referer": "https://example.com"
  }
}
```

---

## Configuration (`%LOCALAPPDATA%\Vajra\config.json`)

Key fields:

```json
{
  "default_output_dir": "C:\\Users\\me\\Downloads",
  "max_concurrent_downloads": 3,
  "default_max_connections": 8,
  "global_speed_limit_bps": 0,
  "listen_port": 6277,
  "notifications_enabled": true,
  "post_queue_action": "none",
  "duplicate_action": "auto_rename",
  "category_rules": [...],
  "proxy": { "url": null, "use_system_proxy": false },
  "blacklist_domains": [],
  "site_connection_limits": {}
}
```

---

## Build Instructions

### Prerequisites
- Rust toolchain (stable, x86_64-pc-windows-msvc)
- Node.js 18+ (for Tauri UI)
- Visual Studio 2022 with C++ workload

### Build all
```bat
build-all.bat
```

### Build only daemon (fast iteration)
```bat
cargo build -p vajra-daemon
```

### Run daemon standalone
```bat
target\debug\vajrad.exe
```

### Run full app
```bat
vajra.bat
```

### Load extension
1. Build the extension: `cd vajra-extension && npm install && npm run build`
2. `chrome://extensions` → Enable Developer Mode
3. Load Unpacked → select `vajra-extension\dist` folder
4. Extension auto-connects when daemon is running

---

## Commercial Packaging & Production Deployment

To ensure commercial readiness, the packaging and compilation pipeline implements professional-grade optimizations:

### 1. Cargo Compiler Optimizations (`Cargo.toml`)
We configure a custom release profile (`[profile.release]`) in the root workspace configuration to produce ultra-lightweight, high-performance binaries:
- **`opt-level = 3`**: Enables aggressive compiler optimizations for speed.
- **`lto = true`**: Performs Link-Time Optimization across the workspace boundary, letting the compiler inline code and optimize cross-crate interfaces.
- **`codegen-units = 1`**: Enables deep compiler analysis to optimize codegen across the entire crate context.
- **`panic = "abort"`**: Reduces panic recovery code overhead, shrinking binary sizes.
- **`strip = true`**: Automatically strips all debug symbols and symbols tables from final executables (shrinking daemon and UI size by over 50%).

### 2. NSIS Installer & OS Integration (`installer.nsi`)
The Nullsoft setup package implements full Windows system integration:
- **System PATH Registration**: The installer checks if the installation folder is present in the registry user environment `Path` variable and appends it. It broadcasts a `WM_WININICHANGE` message to notify running CMD/PowerShell windows immediately.
- **System PATH Cleanup**: Upon uninstallation, the uninstaller strips the installation folder path from the user's `Path` environment variables.
- **App Data Checkbox**: Displays a toggle on uninstall to let users wipe cached configuration files, SQLite download state trackers (`vajra.db`), and transaction logs.
- **Custom Protocol Handling**: Integrates custom protocol registry keys (`vajra://`) to boot or route links straight to the daemon.

---

## Known Issues & Workarounds

### LNK1104 cannot open msvcrt.lib
VS2022 on this machine stores `msvcrt.lib` in `lib\onecore\x64` instead of the standard `lib\x64`.  
**Fix:** Already applied in `.cargo/config.toml`:
```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-L", "C:\\...\\MSVC\\14.51.36231\\lib\\onecore\\x64"]
```

### vswhom-sys build failure
Some Tauri deps pull in `vswhom-sys` (VS detection). If it fails:
```bat
cargo clean
cargo build --workspace
```

---

## Roadmap

- [x] Phase 1: HTTP-based extension connection (no native messaging)
- [x] Phase 2: vajra:// protocol auto-start
- [x] Phase 3: Token bucket throttle, thread stealing, sleep prevention
- [x] Phase 4: Frontend UI — chunk visualizer, speed graph, advanced settings
- [x] Phase 5: Auto-categorization, intelligence, automation & plugins
- [x] Phase 6: Browser Extension & Advanced Features (QUIC, ML ops, content processing, captcha solving)
- [x] Phase 7: yt-dlp integration & Site spider
- [x] Phase 8: System tray (minimize to tray)
- [x] Phase 9: Clipboard monitoring & Automation scripts
