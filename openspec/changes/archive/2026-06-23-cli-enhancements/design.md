## Context

Currently, the `vajra-cli` crate exists but has a very limited set of arguments. It is not fully integrated with the `vajrad` API to perform meaningful download operations. The goal of this change is to enable power users to orchestrate and manage their downloads purely through the command line or via automated shell scripts without ever needing the Tauri UI.

## Goals / Non-Goals

**Goals:**
- Provide subcommands in `vajra-cli` using `clap` (e.g., `add`, `pause`, `resume`, `remove`, `list`, `daemon`).
- Implement an HTTP client within `vajra-cli` (using `reqwest`) to communicate with the `vajrad` REST API.
- Support basic output formatting (e.g., `cli-table` or simple structured text) for the `list` command.

**Non-Goals:**
- Interactive terminal UI (TUI) interfaces (like `ncmpcpp`).
- Adding new backend features to `vajrad` (the CLI will only use existing endpoints).

## Decisions

- **CLI Argument Parsing:** We will use `clap` with the `derive` feature in `vajra-cli`, as it provides excellent subcommands and auto-generated help menus.
- **API Communication:** We will use the `reqwest` blocking client (or async if `vajra-cli` is async) to talk to `http://127.0.0.1:6277/api/v1/`. We will reuse the `schema.rs` definitions from `vajra-daemon` if possible, or duplicate the minimal JSON structures needed.
- **Daemon Management:** 
  - Starting the daemon will use `std::process::Command` to spawn `vajrad` in a detached state.
  - Stopping the daemon will require a new `POST /api/v1/shutdown` endpoint in `vajrad` if one doesn't exist, or it will find and kill the process ID. *Alternative considered:* sending SIGTERM, but cross-platform Windows support for signals is messy, so an API shutdown endpoint is cleaner.

## Risks / Trade-offs

- **Risk:** The daemon port (6277) might change if the user configures it differently.
  **Mitigation:** `vajra-cli` will attempt to read the global `config.json` from `%LOCALAPPDATA%\Vajra\` to find the active port before making requests, falling back to 6277.
- **Risk:** Starting the daemon detached on Windows without popping up a console window.
  **Mitigation:** Use `std::os::windows::process::CommandExt` with `CREATE_NO_WINDOW` flag.
