use crate::apps::AppInfo;
use freedesktop_desktop_entry::{DesktopEntry, Iter};
use rs_apply::Apply;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    discover_desktop_entries().collect()
}

pub fn extract_icon(app: &AppInfo) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    app.icon_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| fs::read(path).map_err(|e| e.into()))
        .transpose()
}

fn discover_desktop_entries() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    Iter::new_for_lang("en")
        .filter_map(Result::ok)
        .filter(is_application_entry)
        .filter_map(|entry| parse_desktop_entry(&entry).transpose())
}

fn is_application_entry(entry: &DesktopEntry) -> bool {
    entry.type_() == Some("Application")
        && entry.no_display() != Some(true)
        && entry.hidden() != Some(true)
}

fn parse_desktop_entry(entry: &DesktopEntry) -> Result<Option<AppInfo>, Box<dyn Error>> {
    let name = entry.name(None).unwrap_or("Unknown Application").to_owned();
    let exec = entry.exec().ok_or("No Exec field in desktop entry")?;

    exec.apply(resolve_executable)?
        .apply(|exe_path| {
            let icon_path = entry.icon().and_then(resolve_icon_path);

            AppInfo {
                name,
                exe_path,
                icon_path,
            }
        })
        .apply(Some)
        .apply(Ok)
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
