use std::error::Error;
use std::path::PathBuf;

#[path = "platforms/windows.rs"]
#[cfg(windows)]
mod windows;

#[path = "platforms/macos.rs"]
#[cfg(target_os = "macos")]
mod macos;

#[path = "platforms/linux.rs"]
#[cfg(target_os = "linux")]
mod linux;

/// Simplified application information
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// Display name of the application
    pub name: String,
    /// Path to the main executable
    pub exe_path: PathBuf,
    /// Optional path to icon file
    pub icon_path: Option<PathBuf>,
}

pub fn discover_applications() -> Result<Vec<AppInfo>, Box<dyn Error>> {
    #[cfg(windows)]
    {
        windows::discover_applications()
    }

    #[cfg(target_os = "macos")]
    {
        macos::discover_applications()
    }

    #[cfg(target_os = "linux")]
    {
        linux::discover_applications()
    }
}

pub fn extract_icon(app: &AppInfo) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    #[cfg(windows)]
    {
        windows::extract_icon(app)
    }

    #[cfg(target_os = "macos")]
    {
        macos::extract_icon(app)
    }

    #[cfg(target_os = "linux")]
    {
        linux::extract_icon(app)
    }
}
