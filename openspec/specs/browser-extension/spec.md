# Browser Extension

## Summary

The Vajra browser extension provides deep integration with Chrome-based (and soon Firefox) browsers, replacing the native download manager and injecting quick-download overlays into media-rich pages.

## Architecture

The extension is built using **React, TypeScript, and Vite**, completely replacing the legacy Vanilla JS approach. It uses Manifest V3 standards.

### Core Responsibilities

1. **Download Interception:**  
   Uses `chrome.downloads.onDeterminingFilename` to capture downloads initiated by the browser. By returning a `suggest({ filename: "" })` block or cancelling the native download, it successfully bypasses Chrome's "Save As" dialogue and routes the download URL directly to the Vajra Daemon (`http://127.0.0.1:6277`).

2. **Advanced Header Parsing:**  
   Uses `chrome.webRequest.onHeadersReceived` to intercept headers (like `Content-Disposition`) early in the download lifecycle. This ensures Vajra receives the correct filename even when the download URL is obfuscated or a blob link.

3. **Media Sniffing & Injection:**  
   A dedicated content script (`content.ts`) is injected into webpages. It scans for `<video>`, `<audio>`, and streaming manifest elements (`m3u8`, `mpd`). Once detected, it overlays a non-intrusive "⚡ Download" or "⚡ Stream Grab" button directly on the player, communicating with the Vajra daemon via HTTP.

4. **Health Monitoring & Auto-Start:**  
   The extension continuously polls the Vajra daemon (`GET /health`). If the daemon is unreachable, the extension UI prompts the user to launch Vajra, which invokes the `vajra://start` protocol handler to boot the desktop application automatically.

5. **Batch Download Checkboxes:**  
   When the user holds the `Alt` key, the extension automatically injects checkboxes next to all anchor `<a>` links. Users can select multiple links and dispatch them as a batch directly to the Vajra Daemon using a dedicated floating Action container.

## UI Components
- **Popup (`popup.tsx`):** A React-based interface reflecting the daemon's connection status, showing active download statistics, and allowing quick queuing of copied links. 
- **Context Menus:** "Download with Vajra" options added to link and media right-click menus, with support for keyboard modifiers (e.g., holding `Alt` to bypass Vajra and use the browser's native downloader).
