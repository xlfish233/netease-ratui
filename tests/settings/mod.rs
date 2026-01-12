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

        // 新增字段
        preload_count: 10,
        audio_cache_max_mb: 4096,
        download_concurrency: Some(4),
        http_timeout_secs: 60,
        http_connect_timeout_secs: 15,
        download_retries: 3,
        download_retry_backoff_ms: 500,
        download_retry_backoff_max_ms: 5000,
    };
    save_settings(data_dir, &s).expect("save_settings");

    let loaded = load_settings(data_dir);
    assert!((loaded.volume - 0.42).abs() < f32::EPSILON);
    assert_eq!(loaded.br, 320_000);
    assert_eq!(loaded.play_mode, "Shuffle");
    assert_eq!(loaded.lyrics_offset_ms, -200);

    // 验证新增字段
    assert_eq!(loaded.preload_count, 10);
    assert_eq!(loaded.audio_cache_max_mb, 4096);
    assert_eq!(loaded.download_concurrency, Some(4));
    assert_eq!(loaded.http_timeout_secs, 60);
    assert_eq!(loaded.http_connect_timeout_secs, 15);
    assert_eq!(loaded.download_retries, 3);
    assert_eq!(loaded.download_retry_backoff_ms, 500);
    assert_eq!(loaded.download_retry_backoff_max_ms, 5000);
}

#[test]
fn settings_default_values() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data_dir = dir.path();

    // 不创建任何配置文件，验证默认值
    let loaded = load_settings(data_dir);

    // 原有字段默认值
    assert_eq!(loaded.volume, 1.0);
    assert_eq!(loaded.br, 999_000);
    assert_eq!(loaded.play_mode, "ListLoop");
    assert_eq!(loaded.lyrics_offset_ms, 0);

    // 新增字段默认值
    assert_eq!(loaded.preload_count, 5);
    assert_eq!(loaded.audio_cache_max_mb, 2048);
    assert_eq!(loaded.download_concurrency, None);
    assert_eq!(loaded.http_timeout_secs, 30);
    assert_eq!(loaded.http_connect_timeout_secs, 10);
    assert_eq!(loaded.download_retries, 2);
    assert_eq!(loaded.download_retry_backoff_ms, 250);
    assert_eq!(loaded.download_retry_backoff_max_ms, 2000);
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
