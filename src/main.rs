use clap::{Parser, Subcommand};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger first
    logs::init_logger()?;

    use daemon::{Message, Response, is_running, run_daemon, send_message};

    match (Cli::parse().command, is_running()) {
        (Commands::Start, false) => {
            logs::log_info("Starting LaunchDock daemon...");
            println!("Starting LaunchDock daemon...");
            run_daemon().await?;
        }
        (Commands::Start, true) => {
            logs::log_info("LaunchDock start requested but already running");
            println!("LaunchDock is already running");
        }
        (Commands::Stop, true) => {
            send_message(Message::Stop).await?;
            logs::log_info("LaunchDock stopped");
            println!("LaunchDock stopped");
        }
        (Commands::Show, true) => {
            send_message(Message::Show).await?;
            logs::log_info("Launcher shown");
            println!("Launcher shown");
        }
        (Commands::Hide, true) => {
            send_message(Message::Hide).await?;
            logs::log_info("Launcher hidden");
            println!("Launcher hidden");
        }
        (Commands::Status, true) => match send_message(Message::Status).await? {
            Response::Status {
                running,
                ui_visible,
            } => {
                logs::log_info(&format!(
                    "Status checked - Daemon: {}, UI: {}",
                    if running { "Running" } else { "Stopped" },
                    if ui_visible { "Visible" } else { "Hidden" }
                ));
                println!("Daemon: {}", if running { "Running" } else { "Stopped" });
                println!("UI: {}", if ui_visible { "Visible" } else { "Hidden" });
            }
            _ => {
                logs::log_error("Status request returned unknown response");
                println!("Status unknown");
            }
        },
        (Commands::Logs { action }, _) => {
            logs::handle_logs_command(action)?;
        }
        (_, false) => {
            logs::log_error("Command attempted but LaunchDock is not running");
            println!("LaunchDock is not running. Use 'launchdock start' to start it.");
        }
    }
    Ok(())
}
