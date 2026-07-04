<div align="center">
  <img src="logo.png" alt="Vajra Logo" width="300"/>

  <h1>Vajra Download Manager</h1>
  <p><strong>The high-performance, next-generation download manager.<br>Unleash the full bandwidth of your connection.</strong></p>

  <p>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/actions/workflows/build.yml">
      <img src="https://img.shields.io/github/actions/workflow/status/msmayanksingh22/Vajra-Download-Manager/build.yml?branch=main&style=for-the-badge&logo=github&color=31c754" alt="Build Status" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest">
      <img src="https://img.shields.io/github/v/release/msmayanksingh22/Vajra-Download-Manager?include_prereleases&style=for-the-badge&logo=github&color=blue" alt="Latest Release" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/releases">
      <img src="https://img.shields.io/github/downloads/msmayanksingh22/Vajra-Download-Manager/total?style=for-the-badge&color=orange" alt="Downloads" />
    </a>
    <a href="https://github.com/msmayanksingh22/Vajra-Download-Manager/blob/main/LICENSE">
      <img src="https://img.shields.io/github/license/msmayanksingh22/Vajra-Download-Manager?style=for-the-badge&color=238636" alt="License" />
    </a>
  </p>
</div>

<br />

Welcome to **Vajra**, a next-generation download manager engineered for ultimate speed and system efficiency. 

Whether you need a blazing-fast user interface for your daily downloads, a terminal CLI for automation, or a browser extension that flawlessly intercepts your clicks, Vajra delivers a premium experience out of the box.

---

## ✨ Why Vajra?

Vajra isn't just another downloader. It uses advanced network and file-system level optimizations to saturate your bandwidth:

- **⚡ Parallel Multiplexing**: Splits files into byte-range segments using concurrent connections, achieving speeds up to **10x faster** than standard browser downloaders.
- **🧠 Connection Stealing**: Dynamically reassigns idle connection threads to assist the slowest active segment, ensuring zero idle workers.
- **💾 OS-Level Pre-allocation**: Instantly allocates space on your hard drive to prevent file fragmentation and zero-filling delays.
- **🚀 Zero-Copy Memory Mapping**: Writes network packages directly to your disk, bypassing traditional memory buffering for minimal RAM usage.
- **🔒 Integrated VPN Kill Switch**: Continuously monitors your network interfaces and automatically pauses active downloads if your VPN connection drops.

---

## 💿 Installation Guide

Getting Vajra installed on your system is incredibly simple. Head over to our [Releases Page](https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest) and download the appropriate installer for your OS:

### 🪟 Windows (10 / 11)
- Download the `.msi` or `.exe` installer.
- Double-click to run the setup wizard.
- Vajra will automatically launch and place a shortcut on your desktop.

### 🍎 macOS (Intel & Apple Silicon)
- Download the `.dmg` file.
- Open the `.dmg` and drag the **Vajra** icon into your `Applications` folder.
- *Note: On first launch, you may need to right-click the app and select "Open" to bypass macOS Gatekeeper.*

### 🐧 Linux (Debian, Ubuntu, Arch, etc.)
- Download the `.AppImage` or `.deb` file.
- **For Debian/Ubuntu**: Double-click the `.deb` file or install via terminal: `sudo apt install ./vajra_*.deb`
- **For AppImage**: Make it executable (`chmod +x Vajra-*.AppImage`) and run it.

---

## 🚀 Getting Started

Vajra comes with three powerful interfaces designed to fit seamlessly into your workflow.

### 1. The Desktop App (UI)
The main window is your control center. When you open Vajra, it starts a lightweight background engine to handle your files.
- Click the **"+" (Add)** button to paste a URL and start a new download.
- Manage your queue: Pause, resume, or cancel active tasks.
- If you close the window, Vajra minimizes to your system tray so your downloads keep running in the background!

### 2. The Browser Extension
For the ultimate convenience, install the Vajra browser extension. It automatically catches files you click on the web and sends them straight to the Vajra Desktop App.

1. Open `chrome://extensions` (or `edge://extensions`) in your browser.
2. Enable **Developer Mode**.
3. Download the [Latest Release](https://github.com/msmayanksingh22/Vajra-Download-Manager/releases/latest) and extract the `vajra-extension.zip` file.
4. Click **Load unpacked** and select the extracted folder.
5. Make sure the Vajra Desktop App is running, and you're good to go!

### 3. The Terminal CLI (`vajra-cli`)
Are you a power user? Automate your workflows directly from your terminal! The `vajra-cli` tool is bundled with the desktop app.

**Basic Download:**
```bash
vajra-cli get "https://example.com/largefile.zip"
```

**Advanced Usage:**
Download a file with 16 concurrent connections to a specific folder:
```bash
vajra-cli get "https://example.com/largefile.zip" \
  --output "C:\Downloads\file.zip" \
  --connections 16 \
  --priority high
```
*(Run `vajra-cli --help` for a full list of available commands and flags)*

---

## 🔌 API Reference (Headless Mode)

For developers and automation enthusiasts, Vajra's background engine exposes a local REST API at `http://127.0.0.1:6277/api/v1`. You can build your own tools on top of Vajra!

- `GET /health` — Check if the daemon is running
- `GET /downloads` — List all active and completed downloads
- `POST /downloads` — Submit a new download task programmatically
- `PATCH /downloads/:id` — Control task state (pause/resume/cancel)
- `GET /stats` — Live global queue throughput and speed statistics
- `GET /events` — Real-time progress updates via Server-Sent Events (SSE)

---

## 🤝 Community & Support

- **Found a bug?** Open an issue on our [GitHub Tracker](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues).
- **Want to contribute code?** Check out our [Developer & Contributor Guide](DEVELOPER.md) to learn how to build Vajra from source and submit Pull Requests!

---

## 🛡️ License & Security

- **License:** Vajra is 100% open source and available under the [GPL-3.0 License](LICENSE).
- **Security:** Please review our [Security Policy](SECURITY.md) to report vulnerabilities privately.
