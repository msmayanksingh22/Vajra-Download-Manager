## 1. Clipboard Monitor in Tauri

- [x] 1.1 Add `tauri-plugin-clipboard-manager` to `vajra-ui-tauri/src-tauri/Cargo.toml`.
- [x] 1.2 Initialize the clipboard plugin in `vajra-ui-tauri/src-tauri/src/lib.rs`.
- [x] 1.3 Add a React `useEffect` in `App.tsx` or a global hook to listen for `tauri://clipboard-changed` events.
- [x] 1.4 Validate clipboard text using a Regex for URLs/Magnets and show a toast "Download Link Detected - Click to Add".

## 2. Auto-Extraction & Post-Processing Scripts

- [x] 2.1 Add `zip` crate to `vajra-daemon/Cargo.toml`.
- [x] 2.2 In `vajra-daemon`, extend `Config` in `config.rs` to include `auto_extract: bool` and `post_process_script: Option<String>`.
- [x] 2.3 In `vajra-daemon/src/engine.rs`, inside the loop where a download finishes, trigger auto-extraction if it's a `.zip`.
- [x] 2.4 In `vajra-daemon/src/engine.rs`, inside the loop where a download finishes, execute the script via `std::process::Command` if configured.

## 3. UI Settings Integration

- [x] 3.1 Update `vajra-ui-tauri/src/windows/SettingsWindow.tsx` to include "Automation" settings (toggles for Clipboard Monitor, Auto-Extract, and Post-Processing Script path).
- [x] 3.2 Add settings persistence for these new options.
