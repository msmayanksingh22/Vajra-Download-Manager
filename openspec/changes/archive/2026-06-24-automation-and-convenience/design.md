## Context

As a modern download manager, Vajra needs features that reduce manual interaction. Polling the clipboard, automatically extracting zip files, and executing custom shell scripts are standard "power user" features in competing tools like IDM or JDownloader.

## Goals / Non-Goals

**Goals:**
- Reliable clipboard polling that doesn't consume excessive CPU.
- Seamless automatic extraction for common formats (.zip).
- Ability to hook arbitrary OS scripts on download completion.

**Non-Goals:**
- Extracting complex multi-part RAR archives (out of scope for v1 of this feature).
- Two-way communication with post-processing scripts (fire and forget only).

## Decisions

- **Clipboard Monitoring in Tauri:** The `tauri-plugin-clipboard-manager` allows the frontend UI to listen to clipboard changes efficiently. This means it only listens when the frontend tray/app is active. The backend daemon shouldn't need a heavy dependency just to read OS clipboards, which is inherently a desktop concern.
- **Auto-Extraction via `compress-tools` or `zip`:** We will use the native Rust `zip` crate to unpack the downloaded artifact if it ends in `.zip`. It will unpack into a sub-folder matching the archive's base name.
- **Post-Processing Scripts via `std::process::Command`:** Upon transition to `Status::Completed`, if the user has configured a script path in settings, the daemon will spawn a background process executing the script, passing the file path as the first argument.

## Risks / Trade-offs

- **Risk: Malicious scripts.** → Mitigation: The script path is strictly set by the user in settings, which requires local filesystem access.
- **Risk: Auto-extract fills disk.** → Mitigation: Leave zip files untouched on disk; do not auto-delete them. Wait for the user to delete them.
