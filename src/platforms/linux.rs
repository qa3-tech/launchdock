use crate::apps::AppInfo;
use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use rs_apply::Apply;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

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
    if icon_name.starts_with('/') {
        return PathBuf::from(icon_name).apply(|p| if p.exists() { Some(p) } else { None });
    }

    ["/usr/share/icons", "/usr/share/pixmaps"]
        .iter()
        .copied()
        .chain(
            std::env::var("HOME")
                .ok()
                .as_ref()
                .map(|home| {
                    vec![
                        format!("{}/.local/share/icons", home),
                        format!("{}/.icons", home),
                    ]
                })
                .unwrap_or_default()
                .iter()
                .map(String::as_str),
        )
        .flat_map(|base_dir| {
            ["png", "svg", "xpm", "ico"].iter().flat_map(move |ext| {
                [
                    format!("{}/hicolor/48x48/apps/{}.{}", base_dir, icon_name, ext),
                    format!("{}/{}.{}", base_dir, icon_name, ext),
                ]
            })
        })
        .map(PathBuf::from)
        .find(|path| path.exists())
}
