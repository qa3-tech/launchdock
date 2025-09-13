use clap::{Parser, Subcommand};
use std::process;

mod daemon;
mod logs;
mod model;
mod view;

#[derive(Parser)]
#[command(name = "launchdock", about = "Cross-platform application launcher")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start,
    Stop,
    Show,
    Hide,
    Status,
    Logs {
        #[command(subcommand)]
        action: Option<logs::LogsAction>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logs::init_logger()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => handle_start(),
        Commands::Stop => send_daemon_command(daemon::DaemonCommand::Stop),
        Commands::Show => {
            if std::env::consts::OS == "macos" {
                handle_show_macos()
            } else {
                send_daemon_command(daemon::DaemonCommand::Show)
            }
        }
        Commands::Hide => send_daemon_command(daemon::DaemonCommand::Hide),
        Commands::Status => send_daemon_command(daemon::DaemonCommand::Status),
        Commands::Logs { action } => {
            logs::handle_logs_command(action)?;
            Ok(())
        }
    }
}

fn handle_start() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we're the daemon process (spawned with null stdio)
    if is_daemon_process() {
        // We ARE the daemon - run it directly
        return daemon::run_daemon();
    }

    // Check if daemon is already running
    if daemon::is_running() {
        println!("LaunchDock is already running");
        return Ok(());
    }

    println!("Starting LaunchDock daemon...");

    // Spawn daemon as separate process
    let exe = std::env::current_exe()?;
    let child = process::Command::new(exe)
        .arg("start")
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .spawn()?;

    println!("Daemon started with PID: {}", child.id());
    Ok(())
}

fn send_daemon_command(cmd: daemon::DaemonCommand) -> Result<(), Box<dyn std::error::Error>> {
    if !daemon::is_running() {
        println!("LaunchDock is not running. Use 'launchdock start' to start it.");
        return Ok(());
    }

    let response = daemon::send_command(cmd)?;

    if !response.success
        && let Some(error) = response.error
    {
        eprintln!("Error: {}", error);
        return Err(error.into());
    }

    match response.data {
        Some(daemon::DaemonResponseData::Status { running, visible }) => {
            println!("Daemon: {}", if running { "Running" } else { "Stopped" });
            println!("UI: {}", if visible { "Visible" } else { "Hidden" });
        }
        None => {
            // Command acknowledged successfully
        }
    }

    Ok(())
}

fn is_daemon_process() -> bool {
    // Check if we're running with null stdio (daemon mode)
    #[cfg(unix)]
    {
        // Check if stdin is a terminal
        unsafe { libc::isatty(0) == 0 }
    }

    #[cfg(windows)]
    {
        // On Windows, check if we have a console window
        use winapi::um::wincon::GetConsoleWindow;
        unsafe { GetConsoleWindow().is_null() }
    }
}

fn handle_show_macos() -> Result<(), Box<dyn std::error::Error>> {
    logs::log_info("Starting UI process");

    let mut model = model::AppModel {
        all_apps: model::discover_apps(),
        ui_visible: true,
    };

    let final_model = view::run_ui(model.clone())?;
    model.ui_visible = final_model.ui_visible;

    logs::log_info("UI process ended");
    Ok(())
}
