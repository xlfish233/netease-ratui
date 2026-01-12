use crate::app::PlayMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // 播放器设置
    pub volume: f32,
    pub br: i64,
    pub play_mode: String,
    pub lyrics_offset_ms: i64,

    // 缓存/预加载设置
    #[serde(default = "default_preload_count")]
    pub preload_count: usize,
    #[serde(default = "default_audio_cache_max_mb")]
    pub audio_cache_max_mb: usize,
    #[serde(default = "default_download_concurrency")]
    pub download_concurrency: Option<usize>,
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
    #[serde(default = "default_http_connect_timeout_secs")]
    pub http_connect_timeout_secs: u64,
    #[serde(default = "default_download_retries")]
    pub download_retries: u32,
    #[serde(default = "default_download_retry_backoff_ms")]
    pub download_retry_backoff_ms: u64,
    #[serde(default = "default_download_retry_backoff_max_ms")]
    pub download_retry_backoff_max_ms: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            br: 999_000,
            play_mode: "ListLoop".to_owned(),
            lyrics_offset_ms: 0,

            // 缓存/预加载默认值
            preload_count: 5,
            audio_cache_max_mb: 2048,
            download_concurrency: None, // None 表示自动检测
            http_timeout_secs: 30,
            http_connect_timeout_secs: 10,
            download_retries: 2,
            download_retry_backoff_ms: 250,
            download_retry_backoff_max_ms: 2000,
        }
    }
}

// 默认值函数（用于 serde default）
fn default_preload_count() -> usize { 5 }
fn default_audio_cache_max_mb() -> usize { 2048 }
fn default_download_concurrency() -> Option<usize> { None }
fn default_http_timeout_secs() -> u64 { 30 }
fn default_http_connect_timeout_secs() -> u64 { 10 }
fn default_download_retries() -> u32 { 2 }
fn default_download_retry_backoff_ms() -> u64 { 250 }
fn default_download_retry_backoff_max_ms() -> u64 { 2000 }

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
