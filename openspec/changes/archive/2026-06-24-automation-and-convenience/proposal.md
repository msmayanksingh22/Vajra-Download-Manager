## Why

"Phase 14: Automation & Convenience" is essential to transition Vajra from a basic downloader to a power-user tool. By adding system-wide clipboard monitoring, auto-extraction of archives, and custom post-processing scripts, users can configure Vajra to fully automate their download pipeline without manual intervention.

## What Changes

- Add a clipboard listener thread in the `vajra-ui-tauri` background or `vajrad` daemon to detect valid URLs (HTTP/HTTPS/Magnet) and present a toast notification/modal to download.
- Integrate a decompression library (e.g., `zip`, `sevenz-rust`) in `vajrad` to automatically unpack completed downloads into their destination directories if the user enables the flag.
- Add an event hook for "Download Completed" in `vajra-engine` to trigger a user-specified bash/batch/powershell script (e.g., moving files, running custom API calls) with the file path as an environment variable or argument.

## Capabilities

### New Capabilities
- `clipboard-monitor`: System-wide clipboard detection of URLs.
- `auto-extraction`: Decompression of .zip, .rar, and .7z upon completion.
- `post-processing-scripts`: Execution of custom OS scripts upon download completion.

### Modified Capabilities


## Impact

- `vajra-daemon` handles post-processing and auto-extraction.
- `vajra-ui-tauri` requires new UI settings panels for managing script triggers, auto-extraction preferences, and clipboard monitoring toggles.
- Introduces new Rust dependencies for clipboard access and archive decompression.
