use crate::app::PlayMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub volume: f32,
    pub br: i64,
    pub play_mode: String,
    pub lyrics_offset_ms: i64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            br: 999_000,
            play_mode: "ListLoop".to_owned(),
            lyrics_offset_ms: 0,
        }
    }
}

pub fn load_settings(data_dir: &Path) -> AppSettings {
    let p = settings_path(data_dir);
    let Ok(bytes) = fs::read(&p) else {
        return AppSettings::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_settings(data_dir: &Path, s: &AppSettings) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let p = settings_path(data_dir);
    let tmp = p.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(s).unwrap_or_else(|_| b"{}".to_vec());
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, &p) {
        let _ = fs::remove_file(&p);
        fs::rename(&tmp, &p).map_err(|_| e)?;
    }
    Ok(())
}

pub fn play_mode_to_string(m: PlayMode) -> String {
    match m {
        PlayMode::Sequential => "Sequential",
        PlayMode::ListLoop => "ListLoop",
        PlayMode::SingleLoop => "SingleLoop",
        PlayMode::Shuffle => "Shuffle",
    }
    .to_owned()
}

pub fn play_mode_from_string(s: &str) -> PlayMode {
    match s {
        "Sequential" => PlayMode::Sequential,
        "SingleLoop" => PlayMode::SingleLoop,
        "Shuffle" => PlayMode::Shuffle,
        _ => PlayMode::ListLoop,
    }
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path();

        let s = AppSettings {
            volume: 0.42,
            br: 320_000,
            play_mode: "Shuffle".to_owned(),
            lyrics_offset_ms: -200,
        };
        save_settings(data_dir, &s).expect("save_settings");

        let loaded = load_settings(data_dir);
        assert!((loaded.volume - 0.42).abs() < f32::EPSILON);
        assert_eq!(loaded.br, 320_000);
        assert_eq!(loaded.play_mode, "Shuffle");
        assert_eq!(loaded.lyrics_offset_ms, -200);
    }

    #[test]
    fn settings_corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path();
        fs::create_dir_all(data_dir).expect("create_dir_all");
        fs::write(settings_path(data_dir), b"{not-json").expect("write");

        let loaded = load_settings(data_dir);
        assert_eq!(loaded.br, AppSettings::default().br);
    }
}
