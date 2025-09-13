use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process, thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

use crate::{
    logs,
    model::{AppModel, discover_apps},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonCommand {
    Show,
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
  #[allow(dead_code)]  
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
    };

    // Channel for UI thread to signal when it's closed
    let mut ui_thread_handle: Option<thread::JoinHandle<()>> = None;

    logs::log_info(&format!(
        "LaunchDock daemon started (PID: {})",
        process::id()
    ));
    logs::log_info(&format!("Socket: {:?}", socket_path()));
    logs::log_info("Daemon ready");

    loop {
        // Check for UI thread completion
        if let Some(ref handle) = ui_thread_handle
            && handle.is_finished()
        {
            model.ui_visible = false;
            if let Some(handle) = ui_thread_handle.take() {
                let _ = handle.join();
            }
            logs::log_info("UI thread closed");
        }

        // Check for incoming connections
        match listener.accept() {
            Ok((mut stream, _)) => {
                let (response, should_stop) = handle_client_connection(&mut stream, &mut model);

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
            // At this point we know daemon is running and UI was not visible
            model.ui_visible = true;

            (
                DaemonResponse::ok_with_data(DaemonResponseData::Status {
                    running: true,
                    visible: true,
                }),
                false,
            )
        }

        DaemonCommand::Status => (
            DaemonResponse::ok_with_data(DaemonResponseData::Status {
                running: is_running(),
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
