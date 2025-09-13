use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::logs;

const MACOS_APP_PATHS: &[&str] = &[
    "/Applications/",
    "/Users/{username}/Applications/",
    "/System/Applications/",
    "/System/Library/CoreServices/",
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
    pub description: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppModel {
    pub all_apps: Vec<App>,
    pub ui_visible: bool,
}

impl AppModel {}

pub fn discover_apps() -> Vec<App> {
    logs::log_info("Discovering applications...");

    let mut apps = Vec::new();
    let mut seen_names = HashSet::new();

    let paths = get_app_paths();

    for path in paths {
        let discovered = scan_directory(&path);
        for app in discovered {
            if seen_names.insert(app.name.clone()) {
                apps.push(app);
            }
        }
    }

    if std::env::consts::OS == "linux" {
        let desktop_apps = discover_desktop_entries();
        for app in desktop_apps {
            if let Some(pos) = apps.iter().position(|a| a.name == app.name) {
                apps[pos] = app;
            } else if seen_names.insert(app.name.clone()) {
                apps.push(app);
            }
        }
    }

    resolve_app_icons(&mut apps);

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    logs::log_info(&format!("Found {} applications", apps.len()));
    apps
}

/// resolves the best available icon for each app
fn resolve_app_icons(apps: &mut [App]) {
    for app in apps.iter_mut() {
        if app.icon.is_none() || !icon_path_exists(&app.icon) {
            app.icon = {
                #[cfg(target_os = "linux")]
                {
                    discover_comprehensive_icon(&app.name)
                }

                #[cfg(target_os = "macos")]
                {
                    discover_comprehensive_icon(&app.path, &app.name)
                }
            };
        }
    }
}

/// Check if the current icon path actually exists
fn icon_path_exists(icon_path: &Option<String>) -> bool {
    icon_path
        .as_ref()
        .map(|path| PathBuf::from(path).exists())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn discover_comprehensive_icon(app_path: &Path, app_name: &str) -> Option<String> {
    // Only process .app bundles for comprehensive search
    if app_path.extension().and_then(|s| s.to_str()) != Some("app") {
        return None;
    }

    let resources_path = app_path.join("Contents/Resources");
    if !resources_path.exists() {
        return None;
    }

    let capitalize_first_letter = |s: &str| -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };

    // Try standard and app-name-based icon locations in order of preference
    let icon_patterns = vec![
        "AppIcon.icns".to_string(),
        format!("{}.icns", app_name),
        format!("{}.icns", app_name.to_lowercase()),
        "app.icns".to_string(),
        "icon.icns".to_string(),
        // Additional common variations
        format!("{}.icns", app_name.to_uppercase()),
        format!("{}.icns", capitalize_first_letter(app_name)),
    ];

    for pattern in icon_patterns {
        let candidate_path = resources_path.join(&pattern);
        if candidate_path.exists() {
            return Some(candidate_path.to_string_lossy().to_string());
        }
    }

    // Search for any .icns file in Resources directory
    if let Ok(entries) = fs::read_dir(&resources_path) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("icns") {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }

    // Last resort: check Info.plist for CFBundleIconFile
    let info_plist_path = app_path.join("Contents/Info.plist");
    if info_plist_path.exists()
        && let Ok(content) = fs::read_to_string(&info_plist_path)
        && let Some(icon_name) = extract_bundle_icon_name(&content)
    {
        let icon_path = resources_path.join(format!("{}.icns", icon_name.trim()));
        if icon_path.exists() {
            return Some(icon_path.to_string_lossy().to_string());
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn extract_bundle_icon_name(plist_content: &str) -> Option<String> {
    // Simple string search for icon file (not a full plist parser)
    if let Some(start) = plist_content.find("<key>CFBundleIconFile</key>")
        && let Some(string_start) = plist_content[start..].find("<string>")
    {
        let string_content_start = start + string_start + 8;
        if let Some(string_end) = plist_content[string_content_start..].find("</string>") {
            let icon_name = &plist_content[string_content_start..string_content_start + string_end];
            return Some(icon_name.to_string());
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn discover_comprehensive_icon(app_name: &str) -> Option<String> {
    let user_icons_path = format!(
        "/home/{}/.local/share/icons/hicolor/48x48/apps/",
        get_username()
    );

    // Linux icon discovery - check standard icon theme directories
    let icon_theme_paths = vec![
        "/usr/share/icons/hicolor/48x48/apps/",
        "/usr/share/icons/hicolor/64x64/apps/",
        "/usr/share/icons/hicolor/128x128/apps/",
        "/usr/share/pixmaps/",
        &user_icons_path,
    ];

    let icon_extensions = vec!["png", "svg", "xpm"];

    for theme_path in icon_theme_paths {
        for ext in &icon_extensions {
            let icon_path =
                PathBuf::from(theme_path).join(format!("{}.{}", app_name.to_lowercase(), ext));
            if icon_path.exists() {
                return Some(icon_path.to_string_lossy().to_string());
            }

            // Try without extension changes
            let icon_path = PathBuf::from(theme_path).join(format!("{}.{}", app_name, ext));
            if icon_path.exists() {
                return Some(icon_path.to_string_lossy().to_string());
            }
        }
    }

    None
}

fn get_app_paths() -> Vec<String> {
    let username = get_username();

    let paths = match std::env::consts::OS {
        "macos" => MACOS_APP_PATHS,
        _ => LINUX_DESKTOP_ENTRY_PATHS,
    };

    paths
        .iter()
        .map(|path| path.replace("{username}", &username))
        .filter(|path| PathBuf::from(path).exists())
        .collect()
}

fn get_username() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("LOGNAME"))
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
                let icon = get_app_icon(&path);
                Some(App {
                    name,
                    path,
                    description: None,
                    icon,
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_app_name(path: &Path) -> String {
    match std::env::consts::OS {
        "macos" => path
            .file_stem()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string(),
        _ => path
            .file_stem()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string(),
    }
}

fn get_app_icon(path: &Path) -> Option<String> {
    match std::env::consts::OS {
        "macos" => get_macos_app_icon(path),
        _ => None, // Linux icons are handled via desktop entries and comprehensive discovery
    }
}

fn get_macos_app_icon(app_path: &Path) -> Option<String> {
    // Only process .app bundles
    if app_path.extension()?.to_str()? != "app" {
        return None;
    }

    // Try the standard AppIcon.icns location first
    let standard_icon_path = app_path.join("Contents/Resources/AppIcon.icns");
    if standard_icon_path.exists() {
        return Some(standard_icon_path.to_string_lossy().to_string());
    }

    None
}

fn is_app(path: &Path) -> bool {
    match std::env::consts::OS {
        "macos" => {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("app"))
                .unwrap_or(false)
                || path.is_file() && is_executable(path)
        }
        _ => path.is_file() && is_executable(path),
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|name| name.to_str())
            .map(|name| name == "bin")
            .unwrap_or(false)
    }
}

fn discover_desktop_entries() -> Vec<App> {
    let username = get_username();
    let mut apps = Vec::new();

    for path_template in LINUX_DESKTOP_ENTRY_PATHS {
        let path = path_template.replace("{username}", &username);
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.filter_map(Result::ok) {
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("desktop")
                    && let Some(app) = parse_desktop_entry(&file_path)
                {
                    apps.push(app);
                }
            }
        }
    }

    apps
}

fn parse_desktop_entry(path: &Path) -> Option<App> {
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

    let exec_path = exec.split_whitespace().next()?.trim_matches('"');

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
        "macos" => Command::new("open").arg(&app.path).spawn(),
        _ => Command::new(&app.path).spawn(),
    };

    if let Err(e) = result {
        logs::log_error(&format!("Failed to launch {}: {}", app.name, e));
    }
}
