use crate::apps::AppInfo;
use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use rs_apply::Apply;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    let mut unique_apps: HashMap<PathBuf, AppInfo> = HashMap::new();

    for result in discover_desktop_entries().chain(special_commands()) {
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
    if let Some(icon_path) = &app.icon_path {
        if !icon_path.exists() {
            return Ok(None);
        }

        // Check file extension
        if let Some(ext) = icon_path.extension().and_then(|e| e.to_str()) {
            match ext {
                "svg" => {
                    // Convert SVG to PNG using resvg
                    let svg_data = fs::read(icon_path)?;
                    let opt = usvg::Options::default();
                    let tree = usvg::Tree::from_data(&svg_data, &opt)?;

                    let size = tree.size();
                    let width = size.width() as u32;
                    let height = size.height() as u32;

                    // Target 48x48 or maintain aspect ratio
                    let (target_w, target_h) = if width > 48 || height > 48 {
                        let scale = 48.0 / width.max(height) as f32;
                        (
                            (width as f32 * scale) as u32,
                            (height as f32 * scale) as u32,
                        )
                    } else {
                        (width, height)
                    };

                    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
                        .ok_or("Failed to create pixmap")?;

                    let scale = target_w as f32 / size.width();
                    let transform = tiny_skia::Transform::from_scale(scale, scale);

                    resvg::render(&tree, transform, &mut pixmap.as_mut());

                    // Convert to PNG bytes
                    pixmap.encode_png().map(Some).map_err(|e| e.into())
                }
                "png" | "jpg" | "jpeg" | "xpm" => {
                    // Read raster image directly
                    fs::read(icon_path).map(Some).map_err(|e| e.into())
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn special_commands() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    let icon_path = [
        "/usr/share/icons/hicolor/scalable/actions/system-shutdown.svg",
        "/usr/share/icons/Adwaita/scalable/actions/system-shutdown-symbolic.svg",
        "/usr/share/pixmaps/system-shutdown.png",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path.exists());

    let username = env::var("USER").ok().filter(|s| !s.is_empty()).or_else(|| {
        Command::new("whoami")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|s| s.trim().to_string())
    });

    let mut commands = vec![
        ("Shutdown", "systemctl poweroff".to_string()),
        ("Restart", "systemctl reboot".to_string()),
        ("Lock Screen", "loginctl lock-session".to_string()),
    ];

    if let Some(user) = username {
        commands.push(("Logout", format!("loginctl terminate-user {}", user)));
    }

    commands.into_iter().map(move |(name, command)| {
        Ok(AppInfo {
            name: name.to_string(),
            exe_path: PathBuf::from(command),
            icon_path: icon_path.clone(),
        })
    })
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

    let base_dirs = get_icon_base_directories();

    // Common icon theme directories
    let icon_themes = [
        "hicolor",         // Universal fallback (always present)
        "Adwaita",         // GNOME, Fedora, many others
        "breeze",          // KDE Plasma
        "breeze-dark",     // KDE Plasma dark variant
        "Yaru",            // Ubuntu
        "Papirus",         // Popular third-party theme
        "Pop",             // Pop!_OS
        "Mint-Y",          // Linux Mint (Cinnamon)
        "elementary-xfce", // XFCE
    ];

    // Common sizes (prioritize scalable SVG, then common PNG sizes)
    let sizes = ["scalable", "48x48", "64x64", "32x32", "24x24", "16x16"];

    // Categories where app icons are found
    let categories = ["apps", "places", "actions", "categories"];

    // Supported extensions (prioritize SVG, fallback to PNG)
    let extensions = ["svg", "png", "xpm"];

    // Search themed icon directories
    for base_dir in &base_dirs {
        let base_path = PathBuf::from(base_dir);
        if !base_path.exists() {
            continue;
        }

        // Skip pixmaps for themed search
        if base_dir.ends_with("pixmaps") {
            continue;
        }

        for theme in icon_themes {
            for size in sizes {
                for category in categories {
                    for ext in extensions {
                        let path = base_path
                            .join(theme)
                            .join(size)
                            .join(category)
                            .join(format!("{}.{}", icon_name, ext));

                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    // Fallback: search pixmaps directory directly
    for base_dir in &base_dirs {
        if base_dir.ends_with("pixmaps") {
            for ext in extensions {
                let path = PathBuf::from(base_dir).join(format!("{}.{}", icon_name, ext));
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn get_icon_base_directories() -> Vec<String> {
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

    base_dirs
}
