//! User preferences shared by UI and background file services.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub compact_shortcut: CompactShortcut,
    pub download_directory: Option<PathBuf>,
    pub work_directory: Option<PathBuf>,
    pub appearance: Appearance,
    pub tray_icon: bool,
    pub close_to_tray: bool,
    pub reduced_motion: bool,
    pub sync_work_outputs: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            compact_shortcut: CompactShortcut::default(),
            download_directory: None,
            work_directory: None,
            appearance: Appearance::System,
            tray_icon: true,
            close_to_tray: false,
            reduced_motion: false,
            sync_work_outputs: false,
        }
    }
}
/// Portable, intentionally limited combinations avoid stealing ordinary typing shortcuts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactShortcut {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}
impl Default for CompactShortcut {
    fn default() -> Self {
        Self {
            key: "a".into(),
            ctrl: !cfg!(target_os = "macos"),
            cmd: cfg!(target_os = "macos"),
            alt: true,
            shift: false,
        }
    }
}
impl CompactShortcut {
    pub fn valid(&self) -> bool {
        let letter = self.key.len() == 1
            && self
                .key
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        let function = self
            .key
            .strip_prefix('f')
            .and_then(|s| s.parse::<u8>().ok())
            .is_some_and(|n| (1..=12).contains(&n));
        (letter || function)
            && (self.ctrl || self.cmd)
            && (self.alt || self.shift)
            && !(self.ctrl && self.shift && self.key == "escape")
    }
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.cmd {
            parts.push(
                if cfg!(target_os = "macos") {
                    "⌘"
                } else {
                    "Win"
                }
                .into(),
            );
        }
        if self.alt {
            parts.push(
                if cfg!(target_os = "macos") {
                    "⌥"
                } else {
                    "Alt"
                }
                .into(),
            );
        }
        if self.shift {
            parts.push("Shift".into());
        }
        parts.push(self.key.to_ascii_uppercase());
        parts.join(" + ")
    }
}
pub fn path() -> PathBuf {
    #[cfg(feature = "ui-capture")]
    if let Some(path) = std::env::var_os("STEPX_SETTINGS_FILE") {
        return PathBuf::from(path);
    }
    crate::resources::user_config().join("stepx-companion/settings.json")
}
pub fn load() -> Settings {
    load_from(&path())
}
fn load_from(path: &Path) -> Settings {
    let mut s: Settings = fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if !s.tray_icon {
        s.close_to_tray = false;
    }
    if !s.compact_shortcut.valid() {
        s.compact_shortcut = CompactShortcut::default();
    }
    s
}
pub fn save(s: &Settings) -> Result<()> {
    save_to(&path(), s)
}
fn save_to(path: &Path, s: &Settings) -> Result<()> {
    ensure!(s.tray_icon || !s.close_to_tray, "请先开启托盘图标");
    ensure!(
        s.compact_shortcut.valid(),
        "快捷键需包含 Ctrl 或 Command，加 Alt 或 Shift，再加字母、数字或 F1–F12"
    );
    let parent = path.parent().context("设置目录无效")?;
    fs::create_dir_all(parent)?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(temp.as_file(), s)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}
pub fn downloads() -> PathBuf {
    load().download_directory.unwrap_or_else(|| {
        crate::resources::user_home()
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Downloads/StepX")
    })
}
pub fn choose_directory(path: &Path) -> Result<PathBuf> {
    let p = path.canonicalize().context("文件夹不存在或无法访问")?;
    ensure!(p.is_dir(), "请选择文件夹");
    let probe = tempfile::NamedTempFile::new_in(&p).context("此文件夹不可写，请选择其他位置")?;
    drop(probe);
    Ok(p)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_and_atomic_replacement() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("settings.json");
        let mut s = Settings::default();
        s.download_directory = Some(t.path().join("中文 Downloads"));
        s.work_directory = Some(t.path().join("项目"));
        s.reduced_motion = true;
        save_to(&p, &s).unwrap();
        assert_eq!(load_from(&p), s);
        s.appearance = Appearance::Dark;
        save_to(&p, &s).unwrap();
        assert_eq!(load_from(&p).appearance, Appearance::Dark);
    }
    #[test]
    fn cannot_hide_without_recoverable_tray() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("settings.json");
        let mut s = Settings::default();
        s.tray_icon = false;
        s.close_to_tray = true;
        assert!(save_to(&p, &s).is_err());
        fs::write(&p, r#"{"tray_icon":false,"close_to_tray":true}"#).unwrap();
        assert!(!load_from(&p).close_to_tray);
    }
    #[test]
    fn shortcut_migrates_and_persists_without_losing_other_settings() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("settings.json");
        fs::write(&p, r#"{"reduced_motion":true}"#).unwrap();
        let mut s = load_from(&p);
        assert_eq!(s.compact_shortcut, CompactShortcut::default());
        s.compact_shortcut.key = "q".into();
        s.compact_shortcut.shift = true;
        save_to(&p, &s).unwrap();
        assert_eq!(load_from(&p), s);
        s.compact_shortcut.key = "enter".into();
        assert!(save_to(&p, &s).is_err());
        assert_eq!(load_from(&p).compact_shortcut.key, "q");
    }
    #[test]
    fn missing_directory_is_not_silently_replaced() {
        let t = tempfile::tempdir().unwrap();
        assert!(choose_directory(&t.path().join("offline-drive")).is_err());
        assert_eq!(choose_directory(t.path()).unwrap(), t.path().canonicalize().unwrap());
    }
}
