use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process, sync::mpsc, thread};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

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
pub enum DaemonResponse {
    Ok,
    Status { running: bool, visible: bool },
}

#[derive(Debug)]
enum InternalEvent {
    ShowUi(AppModel),
    UiClosed,
    Shutdown,
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

pub async fn send_command(cmd: DaemonCommand) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path()).await?;
    let data = serde_json::to_vec(&cmd)?;

    stream.write_all(&data).await?;
    stream.shutdown().await?;

    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await?;

    let response: DaemonResponse = serde_json::from_slice(&buffer)?;
    Ok(response)
}

pub fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(config_dir())?;
    fs::write(pid_file(), process::id().to_string())?;
    
    let (to_main_tx, to_main_rx) = mpsc::channel::<InternalEvent>();
    let (from_main_tx, from_main_rx) = mpsc::channel::<InternalEvent>();
    
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        
        rt.block_on(async {
            if let Err(e) = async_task_loop(to_main_tx, from_main_rx).await {
                logs::log_error(&format!("Async task error: {}", e));
            }
        });
    });
    
    logs::log_info(&format!("LaunchDock daemon started (PID: {})", process::id()));
    
    for event in to_main_rx {
        match event {
            InternalEvent::ShowUi(model) => {
                logs::log_info("Showing UI on main thread");
                
                if let Err(e) = run_ui(model) {
                    logs::log_error(&format!("UI error: {}", e));
                }
                
                // UI has closed, notify async thread
                let _ = from_main_tx.send(InternalEvent::UiClosed);
            }
            InternalEvent::Shutdown => {
                logs::log_info("Shutdown received");
                break;
            }
            _ => {}
        }
    }
    
    cleanup();
    Ok(())
}

async fn async_task_loop(
    to_main: mpsc::Sender<InternalEvent>,
    from_main: mpsc::Receiver<InternalEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = fs::remove_file(socket_path());
    let listener = UnixListener::bind(socket_path())?;
    
    let mut model = AppModel {
        all_apps: discover_apps(),
        ui_visible: false,
        ..Default::default()
    };
    
    logs::log_info("Daemon ready");
    logs::log_info(&format!("Socket: {:?}", socket_path()));
    
    loop {
        tokio::select! {
            result = listener.accept() => {
                if let Ok((mut stream, _)) = result {
                    if let Some(response) = handle_client_connection(&mut stream, &mut model, &to_main).await {
                        let data = serde_json::to_vec(&response)?;
                        let _ = stream.write_all(&data).await;
                    }
                }
            }
            
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                if let Ok(event) = from_main.try_recv() {
                    match event {
                        InternalEvent::UiClosed => {
                            model.ui_visible = false;
                            logs::log_info("UI closed");
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

async fn handle_client_connection(
    stream: &mut UnixStream,
    model: &mut AppModel,
    to_main: &mpsc::Sender<InternalEvent>,
) -> Option<DaemonResponse> {
    let mut buffer = vec![0; 1024];
    let n = stream.read(&mut buffer).await.ok()?;
    let cmd: DaemonCommand = serde_json::from_slice(&buffer[..n]).ok()?;
    
    let response = match cmd {
        DaemonCommand::Show => {
            if !model.ui_visible {
                show_ui(model, to_main);
            }
            DaemonResponse::Ok
        }
        DaemonCommand::Hide => {
            model.ui_visible = false;
            logs::log_info("Hide requested");
            DaemonResponse::Ok
        }
        DaemonCommand::Status => {
            DaemonResponse::Status {
                running: true,
                visible: model.ui_visible,
            }
        }
        DaemonCommand::Stop => {
            logs::log_info("Stop requested");
            let _ = to_main.send(InternalEvent::Shutdown);
            DaemonResponse::Ok
        }
    };
    
    Some(response)
}

fn show_ui(model: &mut AppModel, to_main: &mpsc::Sender<InternalEvent>) {
    model.ui_visible = true;
    model.search_query.clear();
    model.selected_index = 0;
    
    logs::log_info("Requesting UI show");
    let _ = to_main.send(InternalEvent::ShowUi(model.clone()));
}

fn cleanup() {
    logs::log_info("Cleaning up...");
    let _ = fs::remove_file(socket_path());
    let _ = fs::remove_file(pid_file());
}