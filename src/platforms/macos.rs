use crate::apps::AppInfo;
use icns::{IconFamily, IconType};
use plist::Value;
use rs_apply::Apply;
use std::error::Error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    discover_app_bundles().chain(special_commands()).collect()
}

pub fn extract_icon(app: &AppInfo) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    app.icon_path
        .as_ref()
        .filter(|path| path.extension() == Some(std::ffi::OsStr::new("icns")))
        .map(|icon_path| -> Result<Vec<u8>, Box<dyn Error>> {
            File::open(icon_path)?
                .apply(BufReader::new)
                .apply(IconFamily::read)?
                .apply(|icon_family| {
                    [
                        IconType::RGBA32_256x256,
                        IconType::RGBA32_128x128,
                        IconType::RGBA32_64x64,
                        IconType::RGB24_48x48,
                        IconType::RGB24_32x32,
                        // use as last resort as past this res UI performance slows
                        IconType::RGBA32_512x512,
                    ]
                    .iter()
                    .find_map(|&icon_type| icon_family.get_icon_with_type(icon_type).ok())
                    .ok_or("No suitable icon found")
                })?
                .apply(|image| {
                    let mut png_data = Vec::new();
                    image.write_png(&mut png_data)?;
                    Ok(png_data)
                })
        })
        .transpose()
}

fn special_commands() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    const SYSTEM_ICON: &str =
        "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/ToolbarAdvanced.icns";

    [
       (
           "Shutdown",
           "osascript -e 'tell app \"System Events\" to shut down'",
       ),
       (
           "Logout",
           "osascript -e 'tell app \"System Events\" to log out'",
       ),
       (
           "Restart",
           "osascript -e 'tell app \"System Events\" to restart'",
       ),
       (
           "Lock Screen",
           "osascript -e 'tell application \"System Events\" to keystroke \"q\" using {command down, control down}'",
       ),
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

fn app_directories() -> impl Iterator<Item = String> {
    [
        "/Applications".to_string(),
        "/Applications/Utilities".to_string(),
        "/System/Applications".to_string(),
        "/System/Applications/Utilities".to_string(),
    ]
    .into_iter()
    .chain(
        std::env::var("HOME")
            .ok()
            .map(|home| format!("{}/Applications", home)),
    )
}

fn discover_app_bundles() -> impl Iterator<Item = Result<AppInfo, Box<dyn Error>>> {
    app_directories()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("app")))
        .filter_map(|entry| parse_app_bundle(&entry.path()).transpose())
}

fn parse_app_bundle(app_path: &std::path::Path) -> Result<Option<AppInfo>, Box<dyn Error>> {
    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return Ok(None);
    }

    // Extract just the app name from the bundle filename
    let app_name = app_path
        .file_name()
        .ok_or("Invalid app bundle name")?
        .to_string_lossy()
        .trim_end_matches(".app")
        .to_owned();

    let exe_path = app_path.to_path_buf();
    let icon_path = find_icns_icon(app_path, &app_name);

    Ok(Some(AppInfo {
        name: app_name,
        exe_path: exe_path,
        icon_path: icon_path,
    }))
}

fn find_icns_icon(app_path: &std::path::Path, app_name: &str) -> Option<PathBuf> {
    let resources_dir = app_path.join("Contents/Resources");

    // Try patterns first, then plist as last resort
    find_icon_by_patterns(&resources_dir, app_name)
        .or_else(|| get_icon_from_plist(app_path, &resources_dir))
}

fn validate_icns_path(path: &PathBuf) -> bool {
    path.exists()
        && path.metadata().ok().map(|m| m.len() > 0).unwrap_or(false)
        && File::open(path)
            .ok()
            .and_then(|f| IconFamily::read(BufReader::new(f)).ok())
            .is_some()
}

fn find_icon_by_patterns(resources_dir: &std::path::Path, app_name: &str) -> Option<PathBuf> {
    let icon_patterns = [
        format!("{}.icns", app_name),
        format!("{}.icns", app_name.to_lowercase()),
        format!("{}.icns", app_name.to_uppercase()),
        "AppIcon.icns".to_string(),
        "appicon.icns".to_string(),
        "app.icns".to_string(),
        "icon.icns".to_string(),
    ];

    icon_patterns
        .iter()
        .map(|pattern| resources_dir.join(pattern))
        .find(|path| path.exists() && validate_icns_path(path))
}

fn get_icon_from_plist(
    app_path: &std::path::Path,
    resources_dir: &std::path::Path,
) -> Option<PathBuf> {
    let plist_path = app_path.join("Contents/Info.plist");
    let file = File::open(&plist_path).ok()?;
    let plist: Value = plist::from_reader(file).ok()?;
    let dict = plist.as_dictionary()?;

    ["CFBundleIconName", "CFBundleIconFile"]
        .iter()
        .find_map(|&key| {
            let icon_name = dict.get(key)?.as_string()?;
            let icon_filename = if icon_name.ends_with(".icns") {
                icon_name.to_string()
            } else {
                format!("{}.icns", icon_name)
            };
            let icon_path = resources_dir.join(&icon_filename);
            validate_icns_path(&icon_path).then_some(icon_path)
        })
}
