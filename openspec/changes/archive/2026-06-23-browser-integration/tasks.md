## 1. Browser Extension Core
- [x] 1.1 Remove Native Messaging requirements from `browser-extension/manifest.json`.
- [x] 1.2 Update `browser-extension/popup.js` to poll `http://127.0.0.1:6277/health` for connection status.
- [x] 1.3 Update `browser-extension/background.js` to intercept downloads via `chrome.downloads.onDeterminingFilename` and forward to Vajra via HTTP.

## 2. Media Sniffer
- [x] 2.1 Implement network sniffing in `background.js` via `chrome.webRequest` to capture `.m3u8`, `.mpd`, and `.mp4` URLs.
- [x] 2.2 Inject a content script that adds a floating "Download with Vajra" button to web players.
- [x] 2.3 Send the sniffed media URL and headers to Vajra when the button is clicked.

## 3. FFmpeg Muxer
- [x] 3.1 Bundle a lightweight `ffmpeg.exe` into the Tauri resources.
- [x] 3.2 Create `vajra-engine/src/ffmpeg.rs` to invoke FFmpeg for muxing downloaded streams.
- [x] 3.3 Update `vajra-engine/src/download_task.rs` to detect when a download is a segmented media stream and trigger FFmpeg automatically upon completion.
