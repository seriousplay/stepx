//! Locate resources beside a portable executable, inside a macOS bundle, or in source.
use std::path::PathBuf;
pub fn root() -> PathBuf {
    if let Some(path) = std::env::var_os("STEPX_RESOURCE_DIR") { return PathBuf::from(path); }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for path in [dir.to_path_buf(), dir.join("../Resources")] {
                if path.join("scripts").is_dir() { return path; }
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
pub fn user_config() -> PathBuf {
    std::env::var_os("STEPX_CONFIG_DIR").or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("APPDATA")).map(PathBuf::from)
        .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".config"))
}

pub fn python() -> std::ffi::OsString {
    std::env::var_os("STEPX_PYTHON").unwrap_or_else(|| if cfg!(windows) { "python".into() } else { "python3".into() })
}
