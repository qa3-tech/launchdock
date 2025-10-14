use crate::apps::AppInfo;
use rs_apply::Apply;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use winreg::{RegKey, enums::*};

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    registry_apps()
        .chain(program_files_apps())
        .chain(special_commands())
        .collect()
}

pub fn extract_icon(app: &AppInfo) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    app.icon_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|_| Vec::new()) // Placeholder - actual Windows icon extraction would go here
        .apply(Ok)
}

fn special_commands() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    const SYSTEM_ICON: &str = "C:\\Windows\\System32\\shell32.dll";

    [
        ("Shutdown", "shutdown /s /t 0"),
        ("Logout", "shutdown /l"),
        ("Restart", "shutdown /r /t 0"),
        ("Sleep", "rundll32.exe powrprof.dll,SetSuspendState 0,1,0"),
        ("Lock Screen", "rundll32.exe user32.dll,LockWorkStation"),
    ]
    .into_iter()
    .map(|(name, command)| {
        Ok(AppInfo {
            name: name.to_string(),
            exe_path: PathBuf::from(command),
            icon_path: Some(PathBuf::from(SYSTEM_ICON)),
        })
    })
}

fn registry_apps() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER]
        .iter()
        .filter_map(|&hkey| {
            RegKey::predef(hkey)
                .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall")
                .ok()
        })
        .flat_map(|uninstall_key| {
            uninstall_key
                .enum_keys()
                .filter_map(Result::ok)
                .filter_map(move |name| uninstall_key.open_subkey(&name).ok())
                .filter_map(|key| parse_registry_entry(&key).transpose())
        })
}

fn program_files_apps() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    ["C:\\Program Files", "C:\\Program Files (x86)"]
        .iter()
        .filter_map(|&dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_dir()))
        .filter_map(|entry| directory_to_app(&entry.path()).transpose())
}

fn parse_registry_entry(key: &RegKey) -> Result<Option<AppInfo>, Box<dyn Error>> {
    let name = key.get_value::<String, _>("DisplayName").ok();
    let install_location = key
        .get_value::<String, _>("InstallLocation")
        .or_else(|_| key.get_value::<String, _>("UninstallString"))
        .ok()
        .filter(|s| !s.is_empty());

    match (name, install_location) {
        (Some(name), Some(install_location)) => AppInfo {
            name,
            exe_path: PathBuf::from(&install_location),
            icon_path: key
                .get_value::<String, _>("DisplayIcon")
                .ok()
                .map(PathBuf::from),
        }
        .apply(Some)
        .apply(Ok),
        _ => Ok(None),
    }
}

fn directory_to_app(dir: &std::path::Path) -> Result<Option<AppInfo>, Box<dyn Error>> {
    fs::read_dir(dir)?
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("exe")))
        .and_then(|entry| {
            dir.file_name().map(|name| AppInfo {
                name: name.to_string_lossy().into_owned(),
                exe_path: entry.path(),
                icon_path: Some(entry.path()),
            })
        })
        .apply(Ok)
}
