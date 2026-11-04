//! User settings: the six options of the settings panel (docs/ux-spec.md
//! §settings) and nothing more. Stored as JSON; unknown or missing fields
//! fall back to defaults so old files keep working.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnEnd {
    Loop,
    NextFolder,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    None,
    Fade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelMode {
    Navigate,
    Zoom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Slideshow interval in seconds, clamped to 1..=60.
    pub interval_s: u32,
    pub on_end: OnEnd,
    /// Shuffle within the current folder only.
    pub shuffle: bool,
    pub transition: Transition,
    pub start_fullscreen: bool,
    pub wheel: WheelMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval_s: 5,
            on_end: OnEnd::NextFolder,
            shuffle: false,
            transition: Transition::None,
            start_fullscreen: true,
            wheel: WheelMode::Navigate,
        }
    }
}

impl Settings {
    pub fn normalized(mut self) -> Self {
        self.interval_s = self.interval_s.clamp(1, 60);
        self
    }
}

/// Missing or unreadable file -> defaults. A corrupt settings file must never
/// stop the viewer from opening a photo.
pub fn load(path: &Path) -> Settings {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
        .unwrap_or_default()
        .normalized()
}

/// Atomic write (temp file + rename) so a crash never leaves half a file.
pub fn save(path: &Path, s: &Settings) -> Result<()> {
    let json = serde_json::to_vec_pretty(&s.clone().normalized())
        .map_err(|e| crate::Error::corrupt(e.to_string()))?;
    write_atomic(path, &json)
}

/// Temp file + rename, creating the parent folder. Shared with the server's
/// ui-state store.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_and_corrupt_files_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"interval_s": 999, "shuffle": true}"#).unwrap();
        let s = load(&p);
        assert_eq!(s.interval_s, 60);
        assert!(s.shuffle);
        assert_eq!(s.on_end, OnEnd::NextFolder);
        std::fs::write(&p, b"{not json").unwrap();
        assert_eq!(load(&p), Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a").join("settings.json");
        let s = Settings {
            interval_s: 12,
            wheel: WheelMode::Zoom,
            ..Settings::default()
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
    }
}
