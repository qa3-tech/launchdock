use crate::APP_NAME;
use std::fs::{File, metadata};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub fn init_logger() -> Result<(), Box<dyn std::error::Error>> {
    let log_file = get_log_file()?;
    // Create log directory if it doesn't exist
    if let Some(parent) = log_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn log_info(msg: &str) {
    write_log("INFO", msg);
}

pub fn log_error(msg: &str) {
    write_log("ERROR", msg);
}

fn write_log(level: &str, msg: &str) {
    let log_file = match get_log_file() {
        Ok(path) => path,
        Err(_) => return,
    };

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_entry = format!("[{}] {}: {}\n", timestamp, level, msg);

    // Write the log entry
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .and_then(|mut file| file.write_all(log_entry.as_bytes()));

    // Check file size and warn if over 5 MiB (simple check, no mutex needed)
    if let Ok(size) = metadata(&log_file).map(|m| m.len())
        && size > 5 * 1024 * 1024
    {
        let warning = format!(
            "[{}] WARN: Log file is {:.1} MiB. Consider running '{} logs clear'\n",
            timestamp,
            size as f64 / 1_048_576.0,
            APP_NAME
        );
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .and_then(|mut file| file.write_all(warning.as_bytes()));
    }
}

pub fn show_logs(lines: usize) -> Result<(), Box<dyn std::error::Error>> {
    let log_file = get_log_file()?;

    if !log_file.exists() {
        println!("No log file found");
        return Ok(());
    }

    // Show file size warning if needed
    let size = metadata(&log_file)?.len();
    if size > 5 * 1024 * 1024 {
        eprintln!(
            "Warning: Log file is {:.1} MiB. Consider clearing it with '{} logs clear'.",
            size as f64 / 1_048_576.0,
            APP_NAME
        );
    }

    let file = File::open(&log_file)?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

    let start = all_lines.len().saturating_sub(lines);
    for line in &all_lines[start..] {
        println!("{}", line);
    }

    Ok(())
}

pub fn clear_logs() -> Result<(), Box<dyn std::error::Error>> {
    let log_file = get_log_file()?;

    if log_file.exists() {
        std::fs::write(&log_file, "")?;
        println!("Log file cleared");
    } else {
        println!("No log file to clear");
    }

    Ok(())
}

fn get_log_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA")?;

    #[cfg(target_os = "macos")]
    let base = format!("{}/Library/Logs", std::env::var("HOME")?);

    #[cfg(target_os = "linux")]
    let base = std::env::var("XDG_DATA_HOME")
        .or_else(|_| std::env::var("HOME").map(|home| format!("{}/.local/share", home)))?;

    Ok(PathBuf::from(base)
        .join(APP_NAME)
        .join(format!("{}.log", APP_NAME)))
}
