# ⚡ Vajra Download Manager

A high-performance, developer-first download manager. Headless-capable, API-driven, and built entirely in Rust + React.

---


## 🏗 Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│  Browser (Chrome / Edge)                                        │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Vajra Extension v1.1.0                                    │ │
│  │  background.ts ──── intercepts downloads ────────────────► │─┐
│  │  popup.tsx     ──── health poll every 3s ────────────────► │ │
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
│               │  │ Allocator  │  │  Writer          │   │      │
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

## 📦 Workspace Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `vajra-engine` | lib | Core download engine — HTTP/2, multi-segment, thread stealing, resume, retry, throttle |
| `vajra-daemon` | bin (`vajrad`) | REST API server + job queue + SQLite |
| `vajra-protocol` | lib | Shared request/response types and DaemonConfig |
| `vajra-cli` | bin (`vajra`) | Terminal client |
| `vajra-ui-tauri` | app | React + Tauri desktop GUI |
| `vajra-extension` | ext | React + TS Chrome MV3 extension (pure HTTP connection) |

> Note: `vajra-native-host` was removed in v0.4.1. The extension now connects purely via HTTP polling and auto-starts the app via the `vajra://` custom URL protocol.

---

## 🚀 Quick Start

### First-time setup (build once)

```bat
build-all.bat
```

This single script: finds MSVC, builds all Rust crates, installs npm deps, builds the Tauri UI, and verifies everything.

**Prerequisites:**
- [Rust](https://rustup.rs) (`rustup install stable`)
- [Node.js 18+](https://nodejs.org)
- [VS Build Tools 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with "Desktop development with C++"

---

### Running Vajra

```bat
vajra.bat
```

That's it. The UI starts and **automatically launches the daemon** in the background — no separate terminal needed. Close the window → app hides to system tray. Right-click tray → Quit to fully exit.

> Running `vajra.bat` also registers the `vajra://` URL protocol handler in the Windows Registry so the browser extension can auto-start it.

### Browser Extension Setup

1. Open `chrome://extensions` (or `edge://extensions`)
2. Enable **Developer Mode**
3. Click **Load unpacked**
4. Select the `vajra-extension/` folder (or build it via `npm run build` inside `vajra-extension` and select `dist/`)
5. Open the Vajra extension popup. If Vajra isn't running, click **Launch Vajra**. It will auto-connect via `vajra://start`.

---

## ⚙️ Engine Internals (`vajra-engine`)

### Multi-Segment Download Flow

1. **HEAD request** → get `Content-Length`, `Accept-Ranges`, `ETag`
2. **Auto-Categorize** → route to Videos, Music, Docs etc. based on file extension
3. **Allocate file on disk** (sparse file)
   - Windows: `SetEndOfFile` + `SetFileValidData`
4. **Split into N byte-range chunks** (MIN_CHUNK_SIZE = 1 MiB)
5. **Concurrent HTTP GET** — one tokio task per chunk
6. **Dynamic Thread Stealing** — idle threads split the slowest active chunk so no thread ever idles
7. **Bandwidth Throttling** — token bucket limits total and per-download speeds
8. **Memory-Mapped Disk Writer** — chunks written directly to virtual memory-mapped positions via `MmapHandle`, falling back to standard sequential positional disk writing on failure.
9. **State Persistence** — transactional SQLite database state tracking (`download_segments`) for highly resilient resumes.

---

## 🔌 REST API Reference

Base URL: `http://127.0.0.1:6277/api/v1`

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Daemon liveness check |
| `GET` | `/downloads` | List all downloads |
| `POST` | `/downloads` | Add new download (auto-categorized by default) |
| `GET` | `/downloads/:id` | Get single download |
| `PATCH` | `/downloads/:id` | Pause / Resume / Cancel |
| `DELETE`| `/downloads/:id` | Remove download record |
| `GET` | `/stats` | Global throughput and active queue stats |
| `GET` | `/config` | Read current DaemonConfig |
| `PATCH` | `/config` | Update category rules, limits, proxy, etc. |

### SSE (Server-Sent Events)

`GET /events` streams real-time newline-delimited JSON events for progress bars, emitted every 500ms.

---

## 📁 Configuration & Auto-Categorization

The daemon stores config in `%LOCALAPPDATA%/Vajra/config.json`. By default, downloads are automatically routed to the correct folder based on extension:

- **Videos** → `%USERPROFILE%\Videos`
- **Music** → `%USERPROFILE%\Music`
- **Documents** → `%USERPROFILE%\Documents`
- **Software / Archives** → `%USERPROFILE%\Downloads`

You can configure proxy settings, speed limits, max concurrent downloads, and post-queue actions (e.g., sleep/hibernate when done) via the settings UI or API.

---

## 🛠 Troubleshooting

### `LNK1104: cannot open file 'msvcrt.lib'` during build
If you have a non-standard MSVC installation on Windows, the build may fail looking for the CRT libraries.
**Fix:** Add the `onecore` path to `.cargo/config.toml`:
```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-L", "C:\\Program Files\\Microsoft Visual Studio\\...\\lib\\onecore\\x64"]
```

### vswhom-sys build script fails
Some Tauri dependencies try to auto-detect Visual Studio and fail.
**Fix:** Run `cargo clean` and rebuild.

---

## 📖 Further Documentation

- [Architecture & Developer Guide](docs/ARCHITECTURE.md)
- [Changelog](docs/CHANGELOG.md)
