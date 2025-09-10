use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process,
    sync::mpsc,
    thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

use crate::{
    logs,
    model::{AppModel, discover_apps},
    view::run_ui,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonCommand {
    Show,
    Hide,
    Stop,
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub success: bool,
    pub error: Option<String>,
    pub data: Option<DaemonResponseData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponseData {
    Status { running: bool, visible: bool },
}

impl DaemonResponse {
    pub fn ok() -> Self {
        Self {
            success: true,
            error: None,
            data: None,
        }
    }

    pub fn ok_with_data(data: DaemonResponseData) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            error: Some(message),
            data: None,
        }
    }
}

fn config_dir() -> PathBuf {
    match std::env::consts::OS {
        "windows" => {
            let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".to_string());
            PathBuf::from(appdata).join("LaunchDock")
        }
        "macos" => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join("Library/Application Support/LaunchDock")
        }
        _ => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".config/launchdock")
        }
    }
}

fn socket_path() -> PathBuf {
    config_dir().join("launchdockd.sock")
}

fn pid_file() -> PathBuf {
    config_dir().join("launchdockd.pid")
}

pub fn is_running() -> bool {
    let Ok(pid_str) = fs::read_to_string(pid_file()) else {
        return false;
    };

    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };

    is_process_running(pid)
}

#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
    use winapi::um::{
        handleapi::CloseHandle, processthreadsapi::OpenProcess, winnt::PROCESS_QUERY_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
        if handle.is_null() {
            false
        } else {
            CloseHandle(handle);
            true
        }
    }
}

pub fn send_command(cmd: DaemonCommand) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path())?;
    let data = serde_json::to_vec(&cmd)?;

    stream.write_all(&data)?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;

    let response: DaemonResponse = serde_json::from_slice(&buffer)?;
    Ok(response)
}

pub fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(config_dir())?;
    fs::write(pid_file(), process::id().to_string())?;

    // Remove old socket if it exists
    let _ = fs::remove_file(socket_path());

    let listener = UnixListener::bind(socket_path())?;
    listener.set_nonblocking(true)?;

    let mut model = AppModel {
        all_apps: discover_apps(),
        ui_visible: false,
        ..Default::default()
    };

    // Channel for UI thread to signal when it's closed
    let (ui_closed_tx, ui_closed_rx) = mpsc::channel::<()>();
    let mut ui_thread_handle: Option<thread::JoinHandle<()>> = None;

    logs::log_info(&format!(
        "LaunchDock daemon started (PID: {})",
        process::id()
    ));
    logs::log_info(&format!("Socket: {:?}", socket_path()));
    logs::log_info("Daemon ready");

    loop {
        // Check for UI thread completion
        if let Ok(_) = ui_closed_rx.try_recv() {
            model.ui_visible = false;
            if let Some(handle) = ui_thread_handle.take() {
                let _ = handle.join();
            }
            logs::log_info("UI thread closed");
        }

        // Check for incoming connections
        match listener.accept() {
            Ok((mut stream, _)) => {
                let (response, should_stop) = handle_client_connection(
                    &mut stream,
                    &mut model,
                    &ui_closed_tx,
                    &mut ui_thread_handle,
                );

                let response_data = serde_json::to_vec(&response)?;
                let _ = stream.write_all(&response_data);

                if should_stop {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No connection available, sleep briefly
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                logs::log_error(&format!("Accept error: {}", e));
                break;
            }
        }
    }

    cleanup();
    Ok(())
}

fn handle_client_connection(
    stream: &mut UnixStream,
    model: &mut AppModel,
    ui_closed_tx: &mpsc::Sender<()>,
    ui_thread_handle: &mut Option<thread::JoinHandle<()>>,
) -> (DaemonResponse, bool) {
    let mut buffer = vec![0; 1024];

    let n = match stream.read(&mut buffer) {
        Ok(n) => n,
        Err(e) => {
            return (
                DaemonResponse::error(format!("Failed to read command: {}", e)),
                false,
            );
        }
    };

    let cmd: DaemonCommand = match serde_json::from_slice(&buffer[..n]) {
        Ok(cmd) => cmd,
        Err(e) => {
            return (
                DaemonResponse::error(format!("Invalid command format: {}", e)),
                false,
            );
        }
    };

    match cmd {
        DaemonCommand::Show => {
            if model.ui_visible {
                logs::log_info("UI already visible");
                return (DaemonResponse::ok(), false);
            }

            // Clean up any previous UI thread
            if let Some(handle) = ui_thread_handle.take() {
                let _ = handle.join();
            }

            // Reset model state for new UI session
            model.search_query.clear();
            model.selected_index = 0;

            // Clone model for the UI thread
            let ui_model = model.clone();
            let ui_tx = ui_closed_tx.clone();

            // Spawn UI thread
            let handle = thread::spawn(move || {
                logs::log_info("Starting UI thread");

                // Check if we're on main thread (macOS requirement)
                logs::log_info(&format!("UI thread ID: {:?}", thread::current().id()));
                logs::log_info(&format!(
                    "Is main thread: {}",
                    thread::current().name() == Some("main")
                ));

                // Set panic hook to catch UI framework panics
                let original_hook = std::panic::take_hook();
                std::panic::set_hook(Box::new(|panic_info| {
                    logs::log_error(&format!("UI thread panic: {}", panic_info));
                }));

                let result = std::panic::catch_unwind(|| {
                    if let Err(e) = run_ui(ui_model) {
                        logs::log_error(&format!("UI error: {}", e));
                    }
                });

                // Restore original panic hook
                std::panic::set_hook(original_hook);

                if let Err(e) = result {
                    logs::log_error(&format!("UI thread panicked: {:?}", e));
                }

                // Signal that UI has closed
                let _ = ui_tx.send(());
                logs::log_info("UI thread ending");
            });

            *ui_thread_handle = Some(handle);
            model.ui_visible = true;

            logs::log_info("UI show request processed");
            (DaemonResponse::ok(), false)
        }

        DaemonCommand::Hide => {
            logs::log_info("Hide requested - close UI with ESC key or window close");
            (DaemonResponse::ok(), false)
        }

        DaemonCommand::Status => (
            DaemonResponse::ok_with_data(DaemonResponseData::Status {
                running: true,
                visible: model.ui_visible,
            }),
            false,
        ),

        DaemonCommand::Stop => {
            logs::log_info("Stop requested");
            (
                DaemonResponse::ok_with_data(DaemonResponseData::Status {
                    running: false,
                    visible: false,
                }),
                true,
            )
        }
    }
}

fn cleanup() {
    logs::log_info("Cleaning up...");
    let _ = fs::remove_file(socket_path());
    let _ = fs::remove_file(pid_file());
}
