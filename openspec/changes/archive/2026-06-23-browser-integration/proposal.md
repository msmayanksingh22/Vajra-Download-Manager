## Why

A download manager is significantly less useful if users must manually copy and paste URLs. To provide a seamless experience similar to IDM or XDM, Vajra needs a browser extension to intercept downloads automatically and detect media streams on web pages. This enables one-click downloading of both standard files and streaming media.

## What Changes

- Develop a Chrome/Firefox browser extension that communicates with the Vajra Rust Daemon via local HTTP.
- Implement a media sniffer within the extension to detect `video/mp4`, `.m3u8`, and `.mpd` network traffic.
- Inject a floating "Download with Vajra" UI button over media players (e.g., YouTube, Vimeo).
- Integrate a lightweight FFmpeg binary into the Vajra engine to multiplex separate audio and video streams (DASH) or download HLS streams.
- Send download requests from the browser directly to the `vajrad` daemon's REST API.

## Capabilities

### New Capabilities
- `browser-extension`: The browser extension architecture and HTTP communication with the daemon.
- `media-sniffer`: Network interception logic in the extension to detect and extract media stream URLs.
- `ffmpeg-muxer`: Backend FFmpeg integration to handle DASH multiplexing and HLS stream downloading.

### Modified Capabilities
- `download-engine`: Modified to support HLS/DASH media streams and trigger the FFmpeg muxer upon completion.

## Impact

- `browser-extension/` directory will be heavily modified/created to include background scripts and content scripts.
- `vajra-engine/` will gain new dependencies and logic for FFmpeg integration.
- Tauri build system will need to bundle the FFmpeg executable.
