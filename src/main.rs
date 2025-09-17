use std::env;

mod apps;
mod daemon;
mod ipc;
mod logs;
mod ui;

const APP_NAME: &str = "launchdock";

fn print_help() {
    println!("Usage: daemon-app <command> [args]");
    println!();
    println!("Commands:");
    println!("  start         Start the daemon");
    println!("  stop          Stop the daemon");
    println!("  show          Show the UI window");
    println!("  status        Display daemon and UI status");
    println!("  logs          Show recent log entries (default: 50 lines)");
    println!("  logs <n>      Show last n log entries");
    println!("  logs clear    Clear the log file");
}

fn main() {
    // Initialize logger at startup
    if let Err(e) = logs::init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
    }

    let args: Vec<String> = env::args().collect();

    // Check for hidden internal modes first
    if args.len() == 2 && args[1] == "--daemon-mode" {
        daemon::run_daemon_process();
        return;
    }

    if args.len() == 2 && args[1] == "--ui-mode" {
        let result = apps::discover_applications().and_then(|apps| {
            logs::log_info(&format!("Found {} applications", apps.len()));
            ui::run_ui(apps)
        });

        if let Err(e) = result {
            logs::log_error(&format!("Application error: {}", e));
            std::process::exit(1);
        }
        return;
    }

    // Handle public commands
    let result = if args.len() < 2 {
        print_help();
        Ok(())
    } else {
        match args[1].as_str() {
            "start" => daemon::start(),
            "stop" => daemon::stop(),
            "show" => daemon::show(),
            "status" => daemon::status(),
            "logs" => {
                // Handle logs subcommands
                if args.len() > 2 {
                    match args[2].as_str() {
                        "clear" => logs::clear_logs().map_err(|e| e.to_string()),
                        n => {
                            // Try to parse as number
                            match n.parse::<usize>() {
                                Ok(lines) => logs::show_logs(lines).map_err(|e| e.to_string()),
                                Err(_) => Err(format!("Invalid logs argument: {}", n)),
                            }
                        }
                    }
                } else {
                    // Default: show 50 lines
                    logs::show_logs(50).map_err(|e| e.to_string())
                }
            }
            _ => {
                eprintln!("Unknown command: {}", args[1]);
                print_help();
                Err("Invalid command".to_string())
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
