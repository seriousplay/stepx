//! Independent logical sizes for the full and compact shells.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Full,
    MiniAi,
    MiniNotify,
}
impl Mode {
    pub fn compact(self) -> bool {
        self != Self::Full
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
impl Size {
    fn valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && (320.0..=16384.0).contains(&self.width)
            && (300.0..=16384.0).contains(&self.height)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub full: Size,
    pub compact: Size,
    pub mode: Mode,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            full: Size {
                width: 1320.,
                height: 860.,
            },
            compact: Size {
                width: 600.,
                height: 760.,
            },
            mode: Mode::Full,
        }
    }
}
impl Preferences {
    pub fn load(path: &Path) -> Self {
        let mut value: Self = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let defaults = Self::default();
        if !value.full.valid() {
            value.full = defaults.full;
        }
        if !value.compact.valid() {
            value.compact = defaults.compact;
        }
        value
    }
    pub fn size(&self, mode: Mode) -> Size {
        if mode.compact() {
            self.compact
        } else {
            self.full
        }
    }
    pub fn record(&mut self, mode: Mode, size: Option<Size>) {
        self.mode = mode;
        if let Some(size) = size.filter(|s| s.valid()) {
            if mode.compact() {
                self.compact = size;
            } else {
                self.full = size;
            }
        }
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic replacement keeps an interrupted shutdown from corrupting preferences.
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec(self)?)?;
        std::fs::rename(temp, path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_round_trip_without_overwriting_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.json");
        let mut p = Preferences::default();
        p.record(
            Mode::Full,
            Some(Size {
                width: 1440.,
                height: 930.,
            }),
        );
        p.record(
            Mode::MiniAi,
            Some(Size {
                width: 690.,
                height: 810.,
            }),
        );
        p.record(Mode::MiniNotify, None); // A maximized/minimized window must not replace normal size.
        p.save(&path).unwrap();
        let loaded = Preferences::load(&path);
        assert_eq!(p, loaded);
        assert_eq!(loaded.full.width, 1440.);
        assert_eq!(loaded.size(Mode::MiniNotify).width, 690.);
    }
    #[test]
    fn corrupt_and_invalid_sizes_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.json");
        std::fs::write(&path, "broken").unwrap();
        assert_eq!(Preferences::load(&path), Preferences::default());
        let mut p = Preferences::default();
        p.full.width = -1.;
        p.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).full, Preferences::default().full);
        p.record(
            Mode::Full,
            Some(Size {
                width: f32::NAN,
                height: 20.,
            }),
        );
        assert_eq!(p.full.width, -1.);
    }
}
