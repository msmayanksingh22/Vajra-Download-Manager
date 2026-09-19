## 1. CLI Core Updates

- [x] 1.1 Add `reqwest` dependency to `vajra-cli/Cargo.toml`.
- [x] 1.2 Refactor `vajra-cli/src/main.rs` to use `clap` subcommands (`Add`, `Pause`, `Resume`, `Remove`, `List`, `Daemon`).
- [x] 1.3 Implement config loading in `vajra-cli` to read the `vajrad` port from `%LOCALAPPDATA%\Vajra\config.json`.

## 2. API Commands Implementation

- [x] 2.1 Implement `vajra-cli add <url>` using a POST request to `/api/v1/downloads`.
- [x] 2.2 Implement `vajra-cli pause <id>` using a POST request to `/api/v1/downloads/<id>/pause`.
- [x] 2.3 Implement `vajra-cli resume <id>` using a POST request to `/api/v1/downloads/<id>/resume`.
- [x] 2.4 Implement `vajra-cli remove <id>` using a DELETE request to `/api/v1/downloads/<id>`.
- [x] 2.5 Implement `vajra-cli list` using a GET request to `/api/v1/downloads` and format the output as a simple text table.

## 3. Daemon Management Implementation

- [x] 3.1 Implement `vajra-cli daemon status` by making a simple GET request to `/api/v1/ws` (or another health endpoint) to verify connectivity.
- [x] 3.2 Implement `vajra-cli daemon start` using `std::process::Command` to launch `vajrad.exe` in the background without creating a console window.
- [x] 3.3 Implement `vajra-cli daemon stop` using a POST request to `/api/v1/shutdown` (if exists) or by killing the `vajrad` process cleanly.

## 4. Verification & Polish

- [x] 4.1 Verify all commands run correctly against a live daemon.
- [x] 4.2 Verify `vajra-cli daemon start` properly detaches the process.
