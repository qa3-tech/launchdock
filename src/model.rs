use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::PathBuf, process::Command};

use crate::logs;

// Comprehensive application paths
const WINDOWS_APP_PATHS: &[&str] = &[
    "C:\\Program Files\\",
    "C:\\Program Files (x86)\\",
    "C:\\Program Files\\WindowsApps\\",
    "C:\\Users\\{username}\\AppData\\Local\\Programs\\",
    "C:\\Users\\{username}\\AppData\\Roaming\\",
    "C:\\Windows\\System32\\",
    "C:\\Windows\\",
];

const MACOS_APP_PATHS: &[&str] = &[
    "/Applications/",
    "/Users/{username}/Applications/",
    "/System/Applications/",
    "/System/Library/CoreServices/",
    "/usr/bin/",
    "/usr/local/bin/",
    "/opt/homebrew/bin/",
];

const LINUX_APP_PATHS: &[&str] = &[
    "/usr/bin/",
    "/usr/local/bin/",
    "/opt/",
    "/snap/bin/",
    "/var/lib/flatpak/exports/bin/",
    "/home/{username}/.local/bin/",
    "/home/{username}/bin/",
];

const LINUX_DESKTOP_ENTRY_PATHS: &[&str] = &[
    "/usr/share/applications/",
    "/usr/local/share/applications/",
    "/home/{username}/.local/share/applications/",
    "/var/lib/snapd/desktop/applications/",
    "/var/lib/flatpak/exports/share/applications/",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>, // For desktop entries
    pub icon: Option<String>,        // Icon path or name
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppModel {
    pub all_apps: Vec<App>,
    pub search_query: String,
    pub selected_index: usize,
}

impl AppModel {
    pub fn filtered_apps(&self) -> Vec<&App> {
        if self.search_query.is_empty() {
            Vec::new()
        } else {
            self.all_apps
                .iter()
                .filter(|app| {
                    let query = self.search_query.to_lowercase();
                    app.name.to_lowercase().contains(&query)
                        || app
                            .description
                            .as_ref()
                            .map(|desc| desc.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .collect()
        }
    }
}

pub fn discover_apps() -> Vec<App> {
    logs::log_info("Discovering applications...");

    let mut apps = Vec::new();
    let mut seen_names = HashSet::new(); // Deduplicate by name

    // Get all relevant paths for the current OS
    let paths = get_app_paths();

    for path in paths {
        let discovered = scan_directory(&path);
        for app in discovered {
            // Deduplicate by name (keep first occurrence)
            if seen_names.insert(app.name.clone()) {
                apps.push(app);
            }
        }
    }

    // On Linux, prioritize desktop entries over raw executables
    if std::env::consts::OS == "linux" {
        let desktop_apps = discover_desktop_entries();
        for app in desktop_apps {
            // Replace any existing app with same name, as desktop entries have better metadata
            if let Some(pos) = apps.iter().position(|a| a.name == app.name) {
                apps[pos] = app;
            } else if seen_names.insert(app.name.clone()) {
                apps.push(app);
            }
        }
    }

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    logs::log_info(&format!("Found {} applications", apps.len()));
    apps
}

fn get_app_paths() -> Vec<String> {
    let username = get_username();

    let paths = match std::env::consts::OS {
        "windows" => WINDOWS_APP_PATHS,
        "macos" => MACOS_APP_PATHS,
        _ => LINUX_APP_PATHS,
    };

    paths
        .iter()
        .map(|path| path.replace("{username}", &username))
        .filter(|path| PathBuf::from(path).exists()) // Only return existing paths
        .collect()
}

fn get_username() -> String {
    std::env::var("USERNAME") // Windows
        .or_else(|_| std::env::var("USER")) // Unix-like
        .or_else(|_| std::env::var("LOGNAME")) // Alternative Unix
        .unwrap_or_else(|_| "unknown".to_string())
}

fn scan_directory(path: &str) -> Vec<App> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();

            if is_app(&path) {
                let name = get_app_name(&path);
                Some(App {
                    name,
                    path,
                    description: None,
                    icon: None,
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_app_name(path: &PathBuf) -> String {
    match std::env::consts::OS {
        "macos" => {
            // For .app bundles, remove .app extension
            path.file_stem()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .to_string()
        }
        _ => {
            // For other platforms, use file stem
            path.file_stem()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .to_string()
        }
    }
}

fn is_app(path: &PathBuf) -> bool {
    match std::env::consts::OS {
        "windows" => path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("exe"))
            .unwrap_or(false),
        "macos" => {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("app"))
                .unwrap_or(false)
                || path.is_file() && is_executable(path)
        }
        _ => {
            // Linux - check if executable file
            path.is_file() && is_executable(path)
        }
    }
}

fn is_executable(path: &PathBuf) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, assume files in bin directories are executable
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|name| name.to_str())
            .map(|name| name == "bin")
            .unwrap_or(false)
    }
}

// Linux-specific: Parse .desktop files for better app metadata
fn discover_desktop_entries() -> Vec<App> {
    let username = get_username();
    let mut apps = Vec::new();

    for path_template in LINUX_DESKTOP_ENTRY_PATHS {
        let path = path_template.replace("{username}", &username);
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.filter_map(Result::ok) {
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some(app) = parse_desktop_entry(&file_path) {
                        apps.push(app);
                    }
                }
            }
        }
    }

    apps
}

fn parse_desktop_entry(path: &PathBuf) -> Option<App> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut description = None;
    let mut icon = None;
    let mut no_display = false;

    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "Name" => name = Some(value.to_string()),
                "Exec" => exec = Some(value.to_string()),
                "Comment" => description = Some(value.to_string()),
                "Icon" => icon = Some(value.to_string()),
                "NoDisplay" => no_display = value == "true",
                _ => {}
            }
        }
    }

    if no_display {
        return None;
    }

    let name = name?;
    let exec = exec?;

    // Extract the executable path from the Exec line
    let exec_path = exec.split_whitespace().next()?.trim_matches('"');

    // If it's not an absolute path, try to find it in PATH
    let executable_path = if exec_path.starts_with('/') {
        PathBuf::from(exec_path)
    } else {
        find_in_path(exec_path)?
    };

    Some(App {
        name,
        path: executable_path,
        description,
        icon,
    })
}

fn find_in_path(executable: &str) -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for path in path_var.split(':') {
            let full_path = PathBuf::from(path).join(executable);
            if full_path.exists() && is_executable(&full_path) {
                return Some(full_path);
            }
        }
    }
    None
}

pub fn launch_app(app: &App) {
    logs::log_info(&format!("Launching: {}", app.name));
    let result = match std::env::consts::OS {
        "windows" => Command::new(&app.path).spawn(),
        "macos" => Command::new("open").arg(&app.path).spawn(),
        _ => Command::new(&app.path).spawn(),
    };

    if let Err(e) = result {
        logs::log_error(&format!("Failed to launch {}: {}", app.name, e));
    }
}
