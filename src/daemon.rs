use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::mpsc,
};

use crate::{
    logs,
    model::{AppModel, discover_apps},
    view::run_ui,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Message {
    Show,
    Hide,
    Stop,
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Status { running: bool, ui_visible: bool },
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
    config_dir().join("daemon.sock")
}

fn pid_file() -> PathBuf {
    config_dir().join("daemon.pid")
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

pub async fn send_message(msg: Message) -> Result<Response, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path()).await?;
    let data = serde_json::to_vec(&msg)?;

    stream.write_all(&data).await?;
    stream.shutdown().await?;

    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await?;

    let response: Response = serde_json::from_slice(&buffer)?;
    Ok(response)
}

pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    // Create config directory
    fs::create_dir_all(config_dir())?;

    // Remove existing socket if it exists
    let _ = fs::remove_file(socket_path());

    // Bind to Unix socket
    let listener = UnixListener::bind(socket_path())?;

    // Write PID file
    fs::write(pid_file(), process::id().to_string())?;

    // Initialize app model
    let mut model = AppModel {
        all_apps: discover_apps(),
        ..Default::default()
    };
    let mut ui_visible = false;

    // Setup global hotkey
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::META), Code::Escape);
    manager.register(hotkey)?;

    let (hotkey_tx, mut hotkey_rx) = mpsc::channel::<Message>(10);

    // Spawn hotkey monitoring task
    tokio::spawn(async move {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            match receiver.try_recv() {
                Ok(_) => {
                    if hotkey_tx.send(Message::Show).await.is_err() {
                        break;
                    }
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            }
        }
    });

    logs::log_info("LaunchDock daemon started");
    logs::log_info(&format!("PID: {}", process::id()));
    logs::log_info(&format!("Socket: {:?}", socket_path()));
    logs::log_info("Hotkey: Meta+Escape");

    loop {
        tokio::select! {
            // Handle client connections
            result = listener.accept() => {
                match result {
                    Ok((mut stream, _)) => {
                        let mut buffer = vec![0; 1024];
                        if let Ok(n) = stream.read(&mut buffer).await {
                            if let Ok(msg) = serde_json::from_slice::<Message>(&buffer[..n]) {
                                let response = match msg {
                                    Message::Show => {
                                        if !ui_visible {
                                            ui_visible = true;
                                            model.search_query.clear();
                                            model.selected_index = 0;

                                            logs::log_info("Client request: showing launcher");
                                            let model_clone = model.clone();
                                            tokio::spawn(async move {
                                                if let Err(e) = run_ui(model_clone) {
                                                    logs::log_error(&format!("UI error: {}", e));
                                                }
                                            });
                                        }
                                        Response::Ok
                                    }
                                    Message::Hide => {
                                        ui_visible = false;
                                        logs::log_info("Client request: hiding launcher");
                                        Response::Ok
                                    }
                                    Message::Status => {
                                        logs::log_info("Client request: status check");
                                        Response::Status { running: true, ui_visible }
                                    }
                                    Message::Stop => {
                                        logs::log_info("Client request: stopping daemon");
                                        cleanup().await;
                                        return Ok(());
                                    }
                                };

                                let response_data = serde_json::to_vec(&response).unwrap_or_default();
                                let _ = stream.write_all(&response_data).await;
                            }
                        }
                    }
                    Err(e) => {
                        logs::log_error(&format!("Error accepting connection: {}", e));
                    }
                }
            }

            // Handle hotkey events
            Some(msg) = hotkey_rx.recv() => {
                match msg {
                    Message::Show => {
                        if !ui_visible {
                            ui_visible = true;
                            model.search_query.clear();
                            model.selected_index = 0;

                            logs::log_info("Hotkey pressed - showing launcher");
                            let model_clone = model.clone();
                            tokio::spawn(async move {
                                if let Err(e) = run_ui(model_clone) {
                                    logs::log_error(&format!("UI error: {}", e));
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn cleanup() {
    logs::log_info("Cleaning up daemon...");
    let _ = fs::remove_file(socket_path());
    let _ = fs::remove_file(pid_file());
}
