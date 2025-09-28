use crate::apps::AppInfo;
use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use rs_apply::Apply;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    let mut unique_apps: HashMap<PathBuf, AppInfo> = HashMap::new();

    for result in discover_desktop_entries() {
        match result {
            Ok(app) => {
                // Only insert if we haven't seen this executable path before
                // This preserves the first occurrence (highest priority)
                unique_apps.entry(app.exe_path.clone()).or_insert(app);
            }
            Err(_) => continue, // Skip entries with errors
        }
    }

    Ok(unique_apps.into_values().collect())
}

pub fn extract_icon(app: &AppInfo) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    app.icon_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| fs::read(path).map_err(|e| e.into()))
        .transpose()
}

fn discover_desktop_entries() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    Iter::new(default_paths()).filter_map(|path| {
        let content = fs::read_to_string(&path).ok()?;

        if let Ok(entry) = DesktopEntry::decode(&path, &content) {
            if is_application_entry(&entry) {
                let name = entry
                    .name(None)
                    .map(|cow| cow.to_string())
                    .unwrap_or_else(|| "Unknown Application".to_string());
                let exec = entry.exec()?;
                let icon = entry.icon();

                let exe_path = match resolve_executable(exec) {
                    Ok(path) => path,
                    Err(e) => return Some(Err(e)),
                };

                let icon_path = icon.and_then(resolve_icon_path);

                Some(Ok(AppInfo {
                    name,
                    exe_path,
                    icon_path,
                }))
            } else {
                None
            }
        } else {
            None
        }
    })
}

fn is_application_entry(entry: &DesktopEntry) -> bool {
    entry.name(None).is_some() && entry.exec().is_some() && !entry.no_display()
}

fn resolve_executable(exec: &str) -> Result<PathBuf, Box<dyn Error>> {
    exec.split_whitespace()
        .next()
        .unwrap_or(exec)
        .trim_start_matches('"')
        .trim_end_matches('"')
        .apply(|command| {
            if command.starts_with('/') {
                return Ok(PathBuf::from(command));
            }
            std::env::var("PATH")?
                .split(':')
                .map(|dir| PathBuf::from(dir).join(command))
                .find(|path| path.exists())
                .unwrap_or_else(|| PathBuf::from(command))
                .apply(Ok)
        })
}

fn resolve_icon_path(icon_name: &str) -> Option<PathBuf> {
    // Handle absolute paths
    if icon_name.starts_with('/') {
        return PathBuf::from(icon_name).apply(|p| if p.exists() { Some(p) } else { None });
    }

    // Build base directories according to XDG specification
    let mut base_dirs = Vec::new();

    // System directories from XDG_DATA_DIRS (defaults to /usr/local/share:/usr/share)
    env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .filter(|dir| !dir.is_empty())
        .for_each(|dir| base_dirs.push(format!("{}/icons", dir)));

    // Legacy pixmaps directory
    base_dirs.push("/usr/share/pixmaps".to_string());

    // User directories
    if let Ok(home) = env::var("HOME") {
        // XDG_DATA_HOME or default to ~/.local/share
        let data_home =
            env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
        base_dirs.push(format!("{}/icons", data_home));
        base_dirs.push(format!("{}/.icons", home)); // Legacy
    }

    let extensions = ["png", "svg", "xpm"];

    // Search in priority order for best performance
    let searches = [
        // Most common: hicolor theme, standard sizes, apps category
        ("hicolor/48x48/apps", true),
        ("hicolor/32x32/apps", true),
        ("hicolor/24x24/apps", true),
        ("hicolor/scalable/apps", true),
        ("hicolor/16x16/apps", true),
        // Other common categories at standard size
        ("hicolor/48x48/places", true),
        ("hicolor/48x48/actions", true),
        ("hicolor/48x48/mimetypes", true),
        // Direct lookups (for pixmaps and fallbacks)
        ("", false), // Direct in base directory
    ];

    for base_dir in base_dirs {
        let base_path = PathBuf::from(&base_dir);
        if !base_path.exists() {
            continue;
        }

        for (subpath, is_themed) in &searches {
            // For pixmaps directory, only try direct lookup
            if base_dir.ends_with("pixmaps") && *is_themed {
                continue;
            }

            for ext in &extensions {
                let path = if subpath.is_empty() {
                    base_path.join(format!("{}.{}", icon_name, ext))
                } else {
                    base_path
                        .join(subpath)
                        .join(format!("{}.{}", icon_name, ext))
                };

                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    None
}
