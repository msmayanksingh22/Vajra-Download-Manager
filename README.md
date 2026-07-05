<div align="center">
  <img src="logo.png" alt="Vajra Logo" width="220"/>

  <h1>Vajra Download Manager</h1>

  <p><strong>The high-performance, next-generation download manager.<br>Saturate your bandwidth. Take back control.</strong></p>

  <p>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/actions/workflows/build.yml">
      <img src="https://img.shields.io/github/actions/workflow/status/msmayanksingh22/Vajra-Download-Manager/build.yml?branch=main&style=for-the-badge&logo=github&label=CI&color=31c754" alt="CI Status" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest">
      <img src="https://img.shields.io/github/v/release/msmayanksingh22/Vajra-Download-Manager?include_prereleases&style=for-the-badge&logo=github&color=0075FF" alt="Latest Release" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/releases">
      <img src="https://img.shields.io/github/downloads/msmayanksingh22/Vajra-Download-Manager/total?style=for-the-badge&color=orange" alt="Total Downloads" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/blob/main/LICENSE">
      <img src="https://img.shields.io/github/license/msmayanksingh22/Vajra-Download-Manager?style=for-the-badge&color=238636" alt="GPL-3.0 License" />
    </a>
  </p>

  <p>
    <img src="https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust" alt="Rust 1.75+" />
    <img src="https://img.shields.io/badge/Tauri-v2-blue?style=flat-square&logo=tauri" alt="Tauri v2" />
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-informational?style=flat-square" alt="Cross-platform" />
    <img src="https://img.shields.io/badge/Status-Public%20Beta-yellow?style=flat-square" alt="Public Beta" />
  </p>

  <br />

  <p>
    <a href="#-installation">⬇️ Download Now</a> ·
    <a href="#-quick-start-30-seconds">🚀 Quick Start</a> ·
    <a href="#️-using-the-desktop-app">🖥️ Desktop App</a> ·
    <a href="#-browser-extension">🌐 Extension</a> ·
    <a href="#-cli-reference">💻 CLI</a> ·
    <a href="#-headless--api">🔌 API</a> ·
    <a href="DEVELOPER.md">🛠️ Build from Source</a>
  </p>
</div>

---

## 🤔 What Is Vajra?

Vajra is a **free, open-source download manager** — a replacement for your browser's built-in downloader. Instead of downloading files one piece at a time, Vajra splits each file into many pieces and downloads them all at once in parallel, dramatically increasing speed.

Think of it like this: your internet connection is a multi-lane highway, but your browser only uses one lane. Vajra uses all of them.

Beyond speed, Vajra also gives you:
- A **desktop app** with a clean queue and progress view
- A **browser extension** that automatically intercepts downloads
- A **command-line tool** for power users and automation
- A **REST API** for scripting and headless operation
- A **VPN Kill Switch** that pauses downloads if your VPN drops

---

## 📸 Screenshots

> Screenshots are coming soon. The UI is a clean, dark-mode desktop app built with React + Tauri.

<!-- TODO: Add screenshots here once app is running -->
<!-- <img src="docs/screenshot-main.png" alt="Vajra Main Window" width="800"/> -->
<!-- <img src="docs/screenshot-tray.png" alt="Vajra System Tray" width="400"/> -->

---

## 🚀 Quick Start (30 Seconds)

**Just want to download a file faster? Do this:**

1. Go to the **[Latest Release Page](https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest)**
2. Download the installer for your OS (Windows: `.exe`, macOS: `.dmg`, Linux: `.deb`)
3. Install and launch — Vajra appears in your **system tray**
4. Click the `+` button, paste a URL, hit **Start**

That's it. Vajra will download it using parallel connections automatically.

---

## ✨ Why Vajra?

Vajra is not your average download manager. It uses a set of low-level engine optimizations that are typically found only in specialized commercial software:

| Feature | What It Means For You |
|:---|:---|
| **⚡ Parallel Multiplexing** | Downloads are split into segments fetched simultaneously. A 1 Gbps connection that a browser uses at 10% can be pushed to near 100%. |
| **🧠 Connection Stealing** | When one segment finishes early, its connection is instantly reassigned to help the slowest segment. No worker is ever idle. |
| **💾 OS-Level Pre-allocation** | File space is reserved on disk the instant a download starts — no fragmentation, no zero-filling lag, no "disk full" surprises at 99%. |
| **🚀 Zero-Copy Memory Mapping** | Network data is written directly to the correct position in the file on disk. Bypasses system memory buffers, keeping RAM usage flat even for huge files. |
| **🔒 VPN Kill Switch** | Vajra watches your network interfaces. If your VPN drops, all active downloads are paused automatically to prevent traffic leaking outside the VPN tunnel. |
| **🌐 Smart Browser Interception** | A lightweight Chrome/Edge extension intercepts download clicks on any website and hands them off to Vajra instead — no copy-pasting URLs needed. |
| **🤖 Headless & Scriptable** | A full REST API lets you enqueue downloads from shell scripts, CI pipelines, or any custom application. |

---

## 💿 Installation

> **Note:** Vajra is currently in **Public Beta** (`v0.2.x`). Installers are fully functional and ready for daily use.

Head to the **[Latest Release Page](https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest)** and download the installer for your OS.

---

### 🪟 Windows (10 / 11 — 64-bit)

1. Download the **`Vajra_x.x.x_x64-setup.exe`** installer from the Release page.
2. Double-click the `.exe` file to launch the setup wizard.
3. Follow the on-screen steps (Next → Next → Install). Vajra will be installed to `%LocalAppData%\Programs\Vajra`.
4. Vajra will launch automatically after installation and place an icon in your system tray.

> **Windows SmartScreen warning?** This is normal for new open-source software. Click **"More info" → "Run anyway"** to proceed safely.

---

### 🍎 macOS (Intel & Apple Silicon)

1. Download the correct `.dmg` file for your Mac:
   - **Apple Silicon (M1/M2/M3):** `Vajra_x.x.x_aarch64.dmg`
   - **Intel:** `Vajra_x.x.x_x64.dmg`
2. Open the `.dmg` file.
3. Drag the **Vajra** app icon into your **Applications** folder.
4. Launch Vajra from Spotlight (`Cmd + Space` → type "Vajra") or from Applications.

> **"Vajra cannot be opened because Apple cannot check it for malicious software"?**
> This happens because we are not yet enrolled in Apple's paid notarization program.
> To bypass: **Right-click** the Vajra app → **Open** → Click **Open** in the dialog.
> You only need to do this once.

---

### 🐧 Linux

#### Option A: Debian / Ubuntu (`.deb` package)

```bash
# Download the .deb file from the release page, then:
sudo apt install ./Vajra_x.x.x_amd64.deb
```
Vajra will appear in your application launcher.

#### Option B: Universal (`.AppImage`)

```bash
# Download the AppImage from the release page, then:
chmod +x Vajra_x.x.x_amd64.AppImage
./Vajra_x.x.x_amd64.AppImage
```

> **Optional:** Use [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) to integrate the AppImage into your system launcher.

---

## 🖥️ Using the Desktop App

When you first launch Vajra, the desktop app and background engine start together. Here's how to use the main interface:

### Adding a Download

1. Click the **`+` (Add) button** in the top-right corner.
2. Paste the URL of the file you want to download into the URL field.
3. (Optional) Change the output folder, filename, number of connections (default: 8), or speed limit.
4. Click **Start**.

### Managing Your Queue

- **Pause / Resume:** Click the pause ⏸️ or play ▶️ button on any active download row.
- **Cancel:** Click the ✕ button to stop and remove a download (the partial file can optionally be deleted).
- **Priority:** Right-click a download to set it as High, Normal, or Low priority. High priority downloads get more connections assigned to them automatically.

### System Tray

Vajra lives in your **system tray** when the window is closed. Your downloads keep running in the background. Right-click the tray icon to:
- Open the main window
- Check overall download speed
- Pause or resume all downloads
- Quit Vajra

### Settings

Open **Settings** (gear icon) to configure:
- **Default download directory**
- **Maximum simultaneous downloads** (default: 3)
- **Global speed limit**
- **VPN Kill Switch** (on/off + network interface to monitor)
- **Startup behavior** (launch on system boot, start minimized)

---

## 🌐 Browser Extension

The Vajra browser extension intercepts download links on any website and hands them off to the Vajra Desktop App.

### Installation

> **The Vajra Desktop App must already be installed and running** for the extension to work.

1. **Download** `vajra-extension.zip` from the [Latest Release](https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest).
2. **Extract** the zip file to a permanent location (e.g., `C:\Tools\vajra-extension\`).
3. Open your browser and navigate to:
   - **Chrome:** `chrome://extensions`
   - **Edge:** `edge://extensions`
   - **Brave:** `brave://extensions`
4. Enable **"Developer Mode"** using the toggle in the top-right corner.
5. Click **"Load unpacked"** and select the extracted folder.
6. The Vajra icon will appear in your browser toolbar.

### How It Works

- When you click a download link on any website, the extension **automatically intercepts** it and sends it to Vajra instead of the browser's native downloader.
- A small notification will confirm the download was captured.
- You can **click the Vajra extension icon** to:
  - See currently active downloads
  - Pause / resume all downloads
  - Manually submit a URL you have copied to your clipboard

### Extension Settings

Click the extension icon → **Settings** to:
- Configure which file types to auto-intercept (e.g., `.zip`, `.mp4`, `.iso`, `.exe`)
- Set a minimum file size threshold (e.g., only intercept files > 10 MB)
- Temporarily disable interception without uninstalling

---

## 💻 CLI Reference

The `vajra-cli` tool (`vajra` command) provides full control over the Vajra daemon from your terminal. It is bundled alongside the desktop app and should be on your `PATH` after installation.

Verify it is working:
```bash
vajra --version
```

> If `vajra` is not found, you may need to add the Vajra installation directory to your system `PATH`, or run `vajra-cli` directly.

---

### `vajra get` — Download a File

The core command. Adds a URL to the download queue and starts it immediately.

```bash
vajra get <URL> [OPTIONS]
```

**Aliases:** `vajra add`

| Flag | Short | Default | Description |
|:---|:---|:---|:---|
| `--out <DIR>` | `-o` | Default download dir | Directory to save the file into |
| `--filename <NAME>` | `-f` | Auto-detected from URL | Override the output filename |
| `--connections <N>` | `-c` | `8` | Number of parallel connections (1–32) |
| `--limit <BYTES/S>` | | `0` (unlimited) | Speed limit in bytes/second |
| `--priority <LEVEL>` | `-p` | `normal` | Priority: `high`, `normal`, or `low` |
| `--queue-only` | `-q` | off | Add to queue but do not auto-start |
| `--watch` | `-w` | off | Show live progress bar in terminal |
| `--hash <HASH>` | | | Expected SHA-256 or MD5 hash (verified after completion) |
| `--ytdlp` | | off | Force `yt-dlp` for streaming URLs (YouTube, Twitch, etc.) |
| `--auto-extract` | `-x` | off | Automatically extract archive (.zip, .7z, .tar.gz) after download |
| `--script <PATH>` | | | Run a post-processing script after the download completes |
| `--schedule <TIMESTAMP>` | | | Schedule download at a Unix timestamp (e.g., `1720000000`) |
| `--quiet` | | off | Silent mode — no terminal output |

**Examples:**

```bash
# Basic download
vajra get "https://example.com/ubuntu.iso"

# Download with 16 connections and a custom save location
vajra get "https://example.com/ubuntu.iso" --out "/mnt/downloads" --connections 16

# Download, speed-limit to 5 MB/s, and watch progress live
vajra get "https://example.com/bigfile.zip" --limit 5242880 --watch

# Download a YouTube video using yt-dlp
vajra get "https://www.youtube.com/watch?v=dQw4w9WgXcQ" --ytdlp

# Add to queue only (don't start yet), and verify checksum when done
vajra get "https://example.com/file.iso" --queue-only --hash "sha256:abc123..."

# Schedule a download at midnight
vajra get "https://example.com/nightly.zip" --schedule 1720051200
```

---

### `vajra list` — List Downloads

```bash
vajra list [--status <STATUS>]
```

**Alias:** `vajra ls`

| Flag | Default | Options |
|:---|:---|:---|
| `--status <STATUS>` | `all` | `all`, `downloading`, `queued`, `paused`, `complete`, `failed` |

**Examples:**
```bash
vajra list                        # Show all downloads
vajra list --status downloading   # Show only active downloads
vajra list --status failed        # Show failed downloads for review
```

---

### `vajra queue` — Show Priority Queue

Shows the active queue sorted by priority and status (useful for seeing what's next).

```bash
vajra queue
```

---

### `vajra show <ID>` — Inspect a Download

Shows detailed information about a single download: URL, file path, progress, speed, ETA, and errors.

```bash
vajra show <DOWNLOAD_ID>
```

Get the `<DOWNLOAD_ID>` from `vajra list`.

---

### `vajra pause <ID>` — Pause

Pauses an active download while preserving all downloaded segments.

```bash
vajra pause <DOWNLOAD_ID>
```

---

### `vajra resume <ID>` — Resume

Resumes a paused download, picking up exactly where it left off.

```bash
vajra resume <DOWNLOAD_ID>
```

---

### `vajra cancel <ID>` — Cancel

Cancels and removes a download from the queue.

```bash
vajra cancel <DOWNLOAD_ID>
# Also delete the partial file:
vajra cancel <DOWNLOAD_ID> --delete-file
```

---

### `vajra stats` — Global Statistics

Shows a real-time summary of the daemon: active count, speed, uptime.

```bash
vajra stats
```

---

### `vajra inspect <URL>` — Probe a URL

Makes a lightweight HEAD request to a URL and reports: file size, content type, whether byte-range (partial) downloads are supported, and whether `yt-dlp` would be needed. Useful for previewing before downloading.

```bash
vajra inspect "https://example.com/file.zip"
```

---

### `vajra import <FILE>` — Import from IDM (.ef2)

Imports an IDM `.ef2` export file, adding all contained downloads to the queue.

```bash
vajra import "path/to/downloads.ef2"
# Add all to queue without starting:
vajra import "path/to/downloads.ef2" --queue-only
```

---

### `vajra daemon` — Ensure Daemon Is Running

Checks if the background daemon is running. If not, it starts it automatically.

```bash
vajra daemon
```

---

## 🔌 Headless & API

Vajra's daemon exposes a full REST API at `http://127.0.0.1:6277/api/v1`. You can use it to integrate Vajra into any custom workflow, script, or application.

### Interactive API Docs (Swagger UI)

When the daemon is running, open your browser to:
```
http://127.0.0.1:6277/swagger-ui/
```
This gives you a fully interactive interface to explore and test all API endpoints.

### Key Endpoints

| Method | Endpoint | Description |
|:---|:---|:---|
| `GET` | `/health` | Daemon health check — returns `200 OK` if running |
| `GET` | `/api/v1/downloads` | List all downloads (supports `?status=`, `?limit=`, `?offset=`) |
| `POST` | `/api/v1/downloads` | Add a new download task |
| `GET` | `/api/v1/downloads/:id` | Get details for a specific download |
| `PATCH` | `/api/v1/downloads/:id` | Control state: pause, resume, cancel, or update settings |
| `DELETE` | `/api/v1/downloads/:id` | Remove a download |
| `GET` | `/api/v1/stats` | Aggregate daemon statistics |
| `GET` | `/api/v1/events` | Real-time Server-Sent Events (SSE) stream for progress updates |
| `POST` | `/api/v1/inspect` | Probe a URL (returns metadata without downloading) |
| `POST` | `/api/v1/import/ef2` | Import an IDM `.ef2` export file |

### Example: Add a Download via `curl`

```bash
curl -s -X POST http://127.0.0.1:6277/api/v1/downloads \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://example.com/file.zip",
    "output_dir": "/home/user/Downloads",
    "max_connections": 16,
    "priority": "High"
  }' | jq .
```

### Example: Stream Progress via SSE

```bash
curl -N http://127.0.0.1:6277/api/v1/events
```

---

## ❓ Frequently Asked Questions

**Q: Does Vajra replace my browser's downloader entirely?**
A: With the browser extension installed, yes — it intercepts qualifying download links automatically. You can configure which file types are intercepted in the extension settings.

**Q: Is Vajra safe? Will antivirus flag it?**
A: Vajra is 100% open source and the code is auditable. Some antivirus tools flag any new `.exe` that opens network connections, which is a false positive. The full source code is here on GitHub.

**Q: Can Vajra download YouTube videos?**
A: Yes, using the `--ytdlp` flag (CLI) or the "Use yt-dlp" option in the UI. **Note:** This requires [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) to be installed separately and available on your `PATH`.

**Q: What happens if my internet cuts out mid-download?**
A: Vajra saves progress per-segment. When your connection is restored, it resumes automatically from exactly where it left off.

**Q: I want to build Vajra from source — where do I start?**
A: See the [Developer Guide](DEVELOPER.md) for complete, per-OS build instructions.

**Q: The app launched but I don't see a window. Where is it?**
A: Check your **system tray** (bottom-right on Windows, top menu bar on macOS). Vajra runs there by default. Click or right-click the icon to open the main window.

**Q: Is there a portable version that doesn't need installation?**
A: Not yet, but it's planned. For now, use the `.AppImage` on Linux for a near-portable experience.

---

## 🗺️ Roadmap

Vajra is in active development. Here's what's coming:

- [ ] Firefox extension support
- [ ] Torrent / Magnet link support
- [ ] Built-in `yt-dlp` bundling (no separate install needed)
- [ ] Portable (no-install) Windows build
- [ ] Dark/Light theme toggle in settings
- [ ] Download categories and auto-sorting rules

Have an idea? **[Start a Discussion](https://github.com/msmayanksingh22/Vajra-Download-Manager/discussions)** or **[Open an Issue](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues/new)**.

---

## 🧪 Public Beta — We Need Your Help!

Vajra is in **public beta**. This means:
- ✅ Core features are working and stable for daily use
- ⚠️ Some edge cases may not be handled yet
- 🐛 Bugs are expected — please report them!

**The best way to help:** Use Vajra daily and [report any issues](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues/new) you encounter. Every bug report makes Vajra better for everyone.

---

## 🤝 Contributing

We welcome contributions of all sizes — bug reports, documentation improvements, and code patches!

- 🐛 **Report a bug:** [Open an Issue](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues/new)
- 💡 **Suggest a feature:** [Start a Discussion](https://github.com/msmayanksingh22/Vajra-Download-Manager/discussions)
- 🛠️ **Contribute code:** Read the [Developer & Contributor Guide](DEVELOPER.md)

---

## 🛡️ License & Security

- **License:** Vajra is free and open source software, available under the **[GPL-3.0 License](LICENSE)**.
- **Security:** To report a vulnerability privately, see our **[Security Policy](SECURITY.md)**.
