use netease_ratui::settings::{AppSettings, load_settings, save_settings};
use std::fs;

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
    fs::write(data_dir.join("settings.json"), b"{not-json").expect("write");

    let loaded = load_settings(data_dir);
    assert_eq!(loaded.br, AppSettings::default().br);
}
