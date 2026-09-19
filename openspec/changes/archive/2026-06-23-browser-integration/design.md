## Context

Vajra needs to integrate tightly with the browser to offer the one-click download and media detection experience users expect from tools like IDM or XDM. Currently, users have to manually paste URLs into the app.

## Goals / Non-Goals

**Goals:**
- Provide a Chrome/Firefox extension that forwards download requests to Vajra.
- Sniff the network for media streams (video/mp4, .m3u8, .mpd) and inject a floating download button.
- Multiplex audio and video using an embedded FFmpeg binary if the streams are separate.

**Non-Goals:**
- Handling BitTorrent protocol (planned for later).
- Handling DRM-encrypted streams (cannot be downloaded legally/easily without specialized tools).

## Decisions

- **HTTP API over Native Messaging**: We will use the REST API (`http://127.0.0.1:6277/api/v1/downloads`) exposed by `vajrad` to submit downloads. Native Messaging has a higher barrier to entry for cross-platform installation and isn't strictly necessary since `vajrad` already hosts a local server.
- **FFmpeg embedding**: We will ship a lightweight `ffmpeg.exe` binary with the Tauri distribution. When Vajra completes downloading separated video/audio streams or M3U8 chunked streams, it will invoke `ffmpeg` in a subprocess to mux them into a final `.mp4`.
- **Media Sniffer Injection**: The extension will inject a Content Script into video platforms (like YouTube). The background script will listen to `chrome.webRequest` API to capture stream URLs.

## Risks / Trade-offs

- **[Risk]** The browser blocks the local HTTP request due to mixed-content or CORS issues. → **Mitigation**: The extension background script usually has privileges to bypass some CORS, but the daemon must also emit liberal `Access-Control-Allow-Origin: *` headers on the API.
- **[Risk]** FFmpeg increases binary size significantly. → **Mitigation**: Use a stripped-down, minimal build of FFmpeg specifically tailored for basic muxing and stream copying.
