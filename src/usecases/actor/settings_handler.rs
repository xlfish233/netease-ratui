use crate::app::App;
use crate::audio_worker::AudioCommand;
use crate::messages::app::{AppCommand, AppEvent};
use crate::settings;
use crate::usecases::actor::next_song_cache;
use crate::usecases::actor::playback;

use super::utils;
use tokio::sync::mpsc;

const SETTINGS_ITEMS_COUNT: usize = 6;

/// 处理设置相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_settings_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
    next_song_cache: &mut next_song_cache::NextSongCacheManager,
) -> bool {
    match cmd {
        AppCommand::SettingsMoveUp => {
            if matches!(app.view, crate::app::View::Settings) && app.settings_selected > 0 {
                app.settings_selected -= 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::SettingsMoveDown => {
            if matches!(app.view, crate::app::View::Settings) {
                app.settings_selected = (app.settings_selected + 1).min(SETTINGS_ITEMS_COUNT - 1);
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::SettingsDecrease => {
            if matches!(app.view, crate::app::View::Settings) {
                let old_br = app.play_br;
                apply_settings_adjust(app, -1, next_song_cache);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                if tx_audio.send(AudioCommand::SetVolume(app.volume)).is_err() {
                    tracing::warn!("AudioWorker 通道已关闭：SetVolume 发送失败");
                }
                if old_br != app.play_br {
                    let _ = tx_audio.send(AudioCommand::SetCacheBr(app.play_br));
                }
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::SettingsIncrease => {
            if matches!(app.view, crate::app::View::Settings) {
                let old_br = app.play_br;
                apply_settings_adjust(app, 1, next_song_cache);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                if tx_audio.send(AudioCommand::SetVolume(app.volume)).is_err() {
                    tracing::warn!("AudioWorker 通道已关闭：SetVolume 发送失败");
                }
                if old_br != app.play_br {
                    let _ = tx_audio.send(AudioCommand::SetCacheBr(app.play_br));
                }
                utils::push_state(tx_evt, app).await;
            }
        }
        _ => return false,
    }
    true
}

/// 处理设置激活命令（SettingsActivate）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理
pub(super) async fn handle_settings_activate_command(
    app: &mut App,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> Option<bool> {
    if !matches!(app.view, crate::app::View::Settings) {
        return Some(false);
    }

    if is_clear_cache_selected(app) {
        app.settings_status = "正在清除音频缓存...".to_owned();
        tracing::info!("用户触发：清除音频缓存");
        if tx_audio.send(AudioCommand::ClearCache).is_err() {
            tracing::warn!("AudioWorker 通道已关闭：ClearCache 发送失败");
        }
        utils::push_state(tx_evt, app).await;
        Some(true)
    } else if is_logout_selected(app) {
        if !app.logged_in {
            app.settings_status = "未登录，无需退出".to_owned();
            utils::push_state(tx_evt, app).await;
            Some(true)
        } else {
            Some(false) // 由调用者处理登出逻辑
        }
    } else {
        Some(false)
    }
}

/// 处理播放器音量和模式控制命令（涉及设置持久化）
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_player_settings_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
    next_song_cache: &mut next_song_cache::NextSongCacheManager,
) -> bool {
    match cmd {
        AppCommand::PlayerVolumeDown => {
            app.volume = (app.volume - 0.1).clamp(0.0, 2.0);
            if tx_audio.send(AudioCommand::SetVolume(app.volume)).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：SetVolume 发送失败");
            }
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::PlayerVolumeUp => {
            app.volume = (app.volume + 0.1).clamp(0.0, 2.0);
            if tx_audio.send(AudioCommand::SetVolume(app.volume)).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：SetVolume 发送失败");
            }
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::PlayerCycleMode => {
            app.play_mode = playback::next_play_mode(app.play_mode);
            app.play_status = format!("播放模式: {}", playback::play_mode_label(app.play_mode));
            next_song_cache.reset(); // 失效预缓存
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            utils::push_state(tx_evt, app).await;
        }
        _ => return false,
    }
    true
}

/// 从设置同步到 App
pub(super) fn apply_settings_to_app(app: &mut App, s: &settings::AppSettings) {
    app.volume = s.volume.clamp(0.0, 2.0);
    app.play_br = s.br;
    app.play_mode = settings::play_mode_from_string(&s.play_mode);
    app.lyrics_offset_ms = s.lyrics_offset_ms;
}

/// 从 App 同步到设置
pub(super) fn sync_settings_from_app(s: &mut settings::AppSettings, app: &App) {
    s.volume = app.volume;
    s.br = app.play_br;
    s.play_mode = settings::play_mode_to_string(app.play_mode);
    s.lyrics_offset_ms = app.lyrics_offset_ms;
}

fn is_logout_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 1
}

fn is_clear_cache_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 2
}

fn apply_settings_adjust(
    app: &mut App,
    dir: i32,
    next_song_cache: &mut next_song_cache::NextSongCacheManager,
) {
    match app.settings_selected {
        0 => {
            let options = [128_000, 192_000, 320_000, 999_000];
            let pos = options
                .iter()
                .position(|v| *v == app.play_br)
                .unwrap_or(options.len() - 1);
            let next = if dir > 0 {
                (pos + 1).min(options.len() - 1)
            } else {
                pos.saturating_sub(1)
            };
            app.play_br = options[next];
            app.settings_status = format!("音质已设置为 {}", br_label(app.play_br));
        }
        1 => {
            app.volume = (app.volume + if dir > 0 { 0.05 } else { -0.05 }).clamp(0.0, 2.0);
            app.settings_status = format!("音量已设置为 {:.0}%", app.volume * 100.0);
        }
        2 => {
            app.play_mode = if dir > 0 {
                playback::next_play_mode(app.play_mode)
            } else {
                playback::prev_play_mode(app.play_mode)
            };
            app.settings_status = format!("播放模式: {}", playback::play_mode_label(app.play_mode));
            next_song_cache.reset(); // 失效预缓存
        }
        3 => {
            app.lyrics_offset_ms =
                app.lyrics_offset_ms
                    .saturating_add(if dir > 0 { 200 } else { -200 });
            app.settings_status = format!("歌词 offset: {}ms", app.lyrics_offset_ms);
        }
        _ => {}
    }
}

fn br_label(br: i64) -> &'static str {
    match br {
        128_000 => "128k",
        192_000 => "192k",
        320_000 => "320k",
        999_000 => "最高",
        _ => "自定义",
    }
}
