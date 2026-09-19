## Why

Vajra currently has a minimal CLI implementation (`vajra-cli`), but the documentation indicates that "Phase 9: CLI Enhancements" is pending. The lack of an expanded headless argument interface prevents users from interacting properly with Vajra exclusively from the terminal or incorporating Vajra seamlessly into batch scripts and headless servers. We need to expand arguments to include operations like `add`, `pause`, `resume`, `list`, and `remove`.

## What Changes

- Implement `add <url>`, `pause <id>`, `resume <id>`, `list`, and `remove <id>` commands in `vajra-cli`.
- Connect `vajra-cli` commands to the `vajrad` API to issue these operations cleanly without opening the Tauri UI.
- Support options for daemon management (e.g. `start`, `stop`, `status` of the daemon).

## Capabilities

### New Capabilities
- `cli-commands`: Commands to interact with Vajra daemon from the command line (`add`, `pause`, `resume`, `list`, `remove`).
- `headless-daemon-management`: CLI operations to start, stop, and inspect the daemon status.

### Modified Capabilities


## Impact

- `vajra-cli` executable arguments parsing logic.
- Adds HTTP client functionality inside `vajra-cli` to interact with the daemon's existing REST API (`/api/v1/downloads`, `/api/v1/downloads/{id}/pause`, etc.).
