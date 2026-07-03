//! Vajra Tauri shell — auto-manages the vajrad daemon as a sidecar process.
//!
//! On startup: launches vajrad if it isn't already running on port 6277.
//! On window-close: sends SIGTERM/TerminateProcess to the child and waits.
//! On system tray: keeps app alive in tray after closing the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{net::TcpStream, sync::Mutex, time::Duration};

use tauri::{AppHandle, Manager, WindowEvent};
// ── Sidecar handle ─────────────────────────────────────────────────────────────
/// Holds the child process so we can terminate it on exit.
struct DaemonHandle(Mutex<Option<std::process::Child>>);

fn daemon_already_running() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:6277".parse().unwrap(),
        Duration::from_millis(250),
    )
    .is_ok()
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Called from JS: returns current daemon port.
#[tauri::command]
fn daemon_port() -> u16 {
    6277
}

/// Called from JS: true if daemon is responding.
#[tauri::command]
fn daemon_alive() -> bool {
    daemon_already_running()
}

/// Called from JS: dismiss the clipboard suggestion for this URL.
#[tauri::command]
fn dismiss_clipboard_url() { /* frontend just ignores it — no state needed */
}

/// Called from JS: opens the browser setup page in the system default browser.
#[tauri::command]
fn open_browser_setup() {
    let url = "http://127.0.0.1:6277/setup";
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

#[tauri::command]
fn cmd_quit_app(app: AppHandle) {
    quit_app(&app);
}

#[tauri::command]
fn open_file_path(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &path])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn show_in_explorer(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let win_path = path.replace('/', "\\");
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", win_path))
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let path_buf = std::path::PathBuf::from(&path);
        let parent = path_buf.parent().unwrap_or(&path_buf);
        let parent_str = parent.to_string_lossy().to_string();
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&parent_str)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&parent_str)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }
}

// ── App entry ──────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            daemon_port, daemon_alive, open_browser_setup, dismiss_clipboard_url, cmd_quit_app,
            open_file_path, show_in_explorer
        ])
        .setup(|app| {
            // ── Start daemon ───────────────────────────────────────────────────
            log_to_file("tauri-shell.log", "Tauri application setup started");
            let handle = if daemon_already_running() {
                eprintln!("[vajra] daemon already running on :6277");
                log_to_file("tauri-shell.log", "Daemon is already running on port 6277");
                None
            } else {
                eprintln!("[vajra] launching vajrad sidecar…");
                log_to_file("tauri-shell.log", "Launching vajrad sidecar...");
                
                let current_exe = std::env::current_exe().unwrap_or_default();
                let sidecar_path = current_exe.parent().unwrap_or(std::path::Path::new("")).join("vajrad.exe");
                log_to_file("tauri-shell.log", &format!("Trying to launch {:?}", sidecar_path));
                
                let mut cmd = std::process::Command::new(&sidecar_path);
                
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                }
                
                let child = match cmd.spawn() {
                    Ok(child) => {
                        let success_msg = format!("Sidecar process spawned (pid: {:?})", child.id());
                        eprintln!("[vajra] {success_msg}");
                        log_to_file("tauri-shell.log", &success_msg);
                        Some(child)
                    }
                    Err(e) => {
                        let err_msg = format!("ERROR: failed to spawn sidecar at {:?}: {}", sidecar_path, e);
                        eprintln!("[vajra] {err_msg}");
                        log_to_file("tauri-shell.log", &err_msg);
                        use tauri_plugin_dialog::DialogExt;
                        app.handle().dialog().message(err_msg).kind(tauri_plugin_dialog::MessageDialogKind::Error).show(|_| {});
                        None
                    }
                };

                if child.is_some() {
                    // Give it a moment to bind the socket
                    let mut ready = false;
                    for i in 0..40 {
                        std::thread::sleep(Duration::from_millis(250));
                        if daemon_already_running() {
                            ready = true;
                            break;
                        }
                        if i % 4 == 0 {
                            log_to_file("tauri-shell.log", &format!("Waiting for daemon on port 6277... ({}s)", (i as f32) * 0.25));
                        }
                    }
                    if ready {
                        eprintln!("[vajra] daemon ready on :6277");
                        log_to_file("tauri-shell.log", "Daemon ready on port 6277");
                    } else {
                        let err_msg = "daemon process started but port :6277 not responding after 10s";
                        eprintln!("[vajra] {err_msg}");
                        log_to_file("tauri-shell.log", err_msg);
                    }
                } else {
                    let err_msg = "daemon was NOT launched. The UI will show as disconnected.";
                    eprintln!("[vajra] {err_msg}");
                    log_to_file("tauri-shell.log", err_msg);
                }
                child
            };

            app.manage(DaemonHandle(Mutex::new(handle)));

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use futures_util::stream::StreamExt;
                use reqwest_eventsource::{Event, EventSource};
                use tauri::Emitter;

                log_sse("Starting Tauri SSE listener loop");

                let client = reqwest::Client::builder()
                    .tcp_keepalive(Some(std::time::Duration::from_secs(5)))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                loop {
                    log_sse("Connecting to daemon SSE at http://127.0.0.1:6277/api/v1/events");
                    let request_builder = client.get("http://127.0.0.1:6277/api/v1/events");
                    let mut es = match EventSource::new(request_builder) {
                        Ok(source) => {
                            log_sse("EventSource client created successfully");
                            source
                        }
                        Err(e) => {
                            eprintln!("[vajra-sse] EventSource creation error: {}", e);
                            log_sse(&format!("EventSource creation error: {}", e));
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    };

                    while let Some(event) = es.next().await {
                        match event {
                            Ok(Event::Open) => {
                                eprintln!("[vajra-sse] Connected to daemon SSE");
                                log_sse("Connected to daemon SSE successfully");
                            }
                            Ok(Event::Message(message)) => {
                                log_sse(&format!("Received message: event={}, data_len={}", message.event, message.data.len()));
                                if let Ok(mut payload) = serde_json::from_str::<serde_json::Value>(&message.data) {
                                    let mut is_failed = false;
                                    let mut failed_payload = serde_json::Value::Null;

                                    if let Some(obj) = payload.as_object_mut() {
                                        obj.insert("event".to_string(), serde_json::Value::String(message.event.clone()));

                                        let has_failed_status = obj.get("status")
                                            .and_then(|s| s.as_str())
                                            .map(|s| s == "failed")
                                            .unwrap_or(false);

                                        if has_failed_status {
                                            is_failed = true;
                                            failed_payload = serde_json::json!({
                                                "id": obj.get("id").or_else(|| obj.get("download_id")).cloned().unwrap_or(serde_json::Value::Null),
                                                "download_id": obj.get("download_id").or_else(|| obj.get("id")).cloned().unwrap_or(serde_json::Value::Null),
                                                "error": obj.get("error").cloned().unwrap_or(serde_json::Value::String("Download failed".to_string())),
                                            });
                                        }
                                    }

                                    if let Err(e) = app_handle.emit("vajra-event", &payload) {
                                        eprintln!("[vajra-sse] Emit error: {}", e);
                                        log_sse(&format!("Emit error for vajra-event: {}", e));
                                    } else {
                                        log_sse("Successfully emitted vajra-event to frontend");
                                    }

                                    if is_failed {
                                        eprintln!("[vajra-sse] Detected failed download. Emitting DownloadFailed event.");
                                        log_sse("Emitting DownloadFailed event to frontend");
                                        let _ = app_handle.emit("DownloadFailed", &failed_payload);
                                    }
                                } else {
                                    log_sse(&format!("Failed to parse JSON from data: {}", message.data));
                                }
                            }
                            Err(err) => {
                                eprintln!("[vajra-sse] Error: {}", err);
                                log_sse(&format!("SSE stream error: {}", err));
                                es.close();
                                break;
                            }
                        }
                    }
                    log_sse("SSE connection closed or lost. Reconnecting in 2 seconds...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });

            // ── System tray ────────────────────────────────────────────────────
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            use tauri::Emitter;

            let show_i = MenuItem::with_id(app, "show", "Show Vajra", true, None::<&str>)?;
            let add_i = MenuItem::with_id(app, "add_new", "Add New Download...", true, None::<&str>)?;
            let pause_all_i = MenuItem::with_id(app, "pause_all", "Pause All Downloads", true, None::<&str>)?;
            let resume_all_i = MenuItem::with_id(app, "resume_all", "Resume All Downloads", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Vajra", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[
                &show_i,
                &tauri::menu::PredefinedMenuItem::separator(app)?,
                &add_i,
                &pause_all_i,
                &resume_all_i,
                &tauri::menu::PredefinedMenuItem::separator(app)?,
                &quit_i,
            ])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => quit_app(app),
                    "show" => show_window(app),
                    "add_new" => {
                        let _ = app.emit("open-add-url-dialog", ());
                    }
                    "pause_all" => {
                        let _ = app.emit("tray-pause-all", ());
                    }
                    "resume_all" => {
                        let _ = app.emit("tray-resume-all", ());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // Clipboard is handled in React via @tauri-apps/plugin-clipboard-manager

            Ok(())
        })
        // Keep app alive when main window is closed (lives in tray)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    window.hide().unwrap_or_default();
                    api.prevent_close();
                } else {
                    // Destroy secondary windows on close to free their label
                    let _ = window.destroy();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn quit_app(app: &AppHandle) {
    // Kill the daemon sidecar we launched
    if let Some(state) = app.try_state::<DaemonHandle>() {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                eprintln!("[vajra] daemon terminated.");
            }
        }
    }
    app.exit(0);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn log_sse(msg: &str) {
    log_to_file("tauri-sse.log", msg);
}

fn log_to_file(filename: &str, msg: &str) {
    let app_dir = if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(local_app_data).join("Vajra")
    } else {
        std::path::PathBuf::from("C:\\Users\\msmay\\AppData\\Local\\Vajra")
    };
    let _ = std::fs::create_dir_all(&app_dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(app_dir.join(filename))
    {
        use std::io::Write;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}
