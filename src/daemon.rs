use crate::logs;
use std::env;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::ipc::{
    Command as IpcCommand, DAEMON_ADDR, Response, messages, pid_file_path, send_command,
    send_response,
};

// Public API functions that main.rs calls

pub fn start() -> Result<(), String> {
    if is_running() {
        logs::log_info("Attempted to start daemon but already running");
        return Err(messages::DAEMON_ALREADY_RUNNING.to_string());
    }

    let exe = env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

    Command::new(exe)
        .arg("--daemon-mode")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start daemon: {}", e))?;

    // Wait for daemon to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    if is_running() {
        logs::log_info("Daemon started successfully");
        println!("{}", messages::DAEMON_STARTED);
        Ok(())
    } else {
        logs::log_error("Failed to start daemon");
        Err(messages::FAILED_TO_START.to_string())
    }
}

pub fn stop() -> Result<(), String> {
    if !is_running() {
        return Err(messages::DAEMON_NOT_RUNNING.to_string());
    }

    match send_command(IpcCommand::Stop) {
        Ok(Response::Ok(msg)) => {
            println!("{}", msg);
            Ok(())
        }
        Ok(Response::Error(e)) => Err(e),
        Ok(_) => Err(messages::INVALID_RESPONSE.to_string()),
        Err(e) => Err(format!("{}: {}", messages::FAILED_TO_COMMUNICATE, e)),
    }
}

pub fn show() -> Result<(), String> {
    if !is_running() {
        return Err(messages::DAEMON_NOT_RUNNING.to_string());
    }

    match send_command(IpcCommand::Show) {
        Ok(Response::Ok(msg)) => {
            println!("{}", msg);
            Ok(())
        }
        Ok(Response::Error(e)) => Err(e),
        Ok(_) => Err(messages::INVALID_RESPONSE.to_string()),
        Err(e) => Err(format!("{}: {}", messages::FAILED_TO_COMMUNICATE, e)),
    }
}

pub fn status() -> Result<(), String> {
    if !is_running() {
        println!("Daemon: not running");
        println!("UI: not visible");
        return Ok(());
    }

    match send_command(IpcCommand::Status) {
        Ok(Response::Status {
            daemon_running,
            ui_visible,
        }) => {
            println!(
                "Daemon: {}",
                if daemon_running {
                    "running"
                } else {
                    "not running"
                }
            );
            println!("UI: {}", if ui_visible { "visible" } else { "not visible" });
            Ok(())
        }
        Ok(_) => Err(messages::INVALID_RESPONSE.to_string()),
        Err(e) => Err(format!("{}: {}", messages::FAILED_TO_COMMUNICATE, e)),
    }
}

pub fn is_running() -> bool {
    let pid_path = pid_file_path();
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, 0) == 0
            }
            #[cfg(not(unix))]
            true
        } else {
            false
        }
    } else {
        false
    }
}

// Internal daemon implementation

struct DaemonState {
    ui_process: Option<Child>,
    ui_visible: bool,
}

enum Message {
    ShowUI,
    CheckStatus,
    Shutdown,
}

impl DaemonState {
    fn new() -> Self {
        Self {
            ui_process: None,
            ui_visible: false,
        }
    }

    fn update(&mut self, msg: Message) -> Response {
        match msg {
            Message::ShowUI => {
                if self.ui_visible {
                    Response::Ok(messages::UI_ALREADY_VISIBLE.to_string())
                } else {
                    self.launch_ui()
                }
            }
            Message::CheckStatus => Response::Status {
                daemon_running: true,
                ui_visible: self.ui_visible,
            },
            Message::Shutdown => {
                if let Some(mut child) = self.ui_process.take() {
                    let _ = child.kill();
                }
                Response::Ok(messages::DAEMON_STOPPING.to_string())
            }
        }
    }

    fn launch_ui(&mut self) -> Response {
        let exe = match env::current_exe() {
            Ok(path) => path,
            Err(e) => return Response::Error(format!("Failed to get executable: {}", e)),
        };

        match Command::new(exe)
            .arg("--ui-mode")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                logs::log_info("UI process launched");
                self.ui_process = Some(child);
                self.ui_visible = true;
                Response::Ok(messages::UI_LAUNCHED.to_string())
            }
            Err(e) => {
                logs::log_error(&format!("Failed to launch UI: {}", e));
                Response::Error(format!("Failed to launch UI: {}", e))
            }
        }
    }

    fn poll_ui_status(&mut self) {
        // Direct state mutation is appropriate here since we're polling
        // subprocess status, not handling user-triggered events
        if let Some(ref mut child) = self.ui_process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    logs::log_info(&format!("UI process exited with status: {}", status));
                    self.ui_visible = false;
                    self.ui_process = None;
                }
                Ok(None) => {}
                Err(e) => {
                    logs::log_error(&format!("Error checking UI process status: {}", e));

                    self.ui_visible = false;
                    self.ui_process = None;
                }
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, state: Arc<Mutex<DaemonState>>) -> bool {
    // Read single byte command
    let mut cmd_byte = [0u8; 1];
    if stream.read_exact(&mut cmd_byte).is_err() {
        return false;
    }

    let Some(cmd) = IpcCommand::from_byte(cmd_byte[0]) else {
        // Send error response for unknown command
        let _ = send_response(&mut stream, &Response::Error("Unknown command".to_string()));
        return false;
    };

    // Process command and generate response
    let (response, should_exit) = {
        let mut state = state.lock().unwrap();
        state.poll_ui_status();

        match cmd {
            IpcCommand::Stop => {
                logs::log_info("Received stop command");
                let resp = state.update(Message::Shutdown);
                (resp, true) // Signal to exit
            }
            IpcCommand::Show => {
                logs::log_info("Received show command");
                (state.update(Message::ShowUI), false)
            }
            IpcCommand::Status => (state.update(Message::CheckStatus), false),
        }
    };

    // Send response
    let _ = send_response(&mut stream, &response);

    should_exit
}

pub fn run_daemon_process() {
    logs::log_info("Daemon process starting");

    // Write PID file
    let pid_path = pid_file_path();
    if let Err(e) = std::fs::write(&pid_path, std::process::id().to_string()) {
        eprintln!("Failed to write PID file: {}", e);
        return;
    }

    let state = Arc::new(Mutex::new(DaemonState::new()));

    // Start UI status monitor thread
    let monitor_state = Arc::clone(&state);
    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
            let mut state = monitor_state.lock().unwrap();
            state.poll_ui_status();
        }
    });

    // Start TCP listener
    let listener = match TcpListener::bind(DAEMON_ADDR) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {}: {}", DAEMON_ADDR, e);
            let _ = std::fs::remove_file(&pid_path);
            return;
        }
    };

    // Main daemon loop
    for stream in listener.incoming() {
        if let Ok(stream) = stream
            && handle_client(stream, Arc::clone(&state))
        {
            break; // Stop command received
        }
    }

    // Cleanup
    logs::log_info("Daemon process shutting down");
    let _ = std::fs::remove_file(&pid_path);
}
