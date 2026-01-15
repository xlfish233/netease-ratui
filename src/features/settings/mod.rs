use crate::core::prelude::{
    app::App, audio::AudioCommand, effects::CoreEffects, infra::NextSongCacheManager,
    messages::AppCommand,
};
use crate::settings;

const SETTINGS_ITEMS_COUNT: usize = 7;

/// 处理设置相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub async fn handle_settings_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    effects: &mut CoreEffects,
    next_song_cache: &mut NextSongCacheManager,
) -> bool {
    match cmd {
        AppCommand::SettingsMoveUp => {
            if matches!(app.view, crate::app::View::Settings) && app.settings_selected > 0 {
                app.settings_selected -= 1;
                effects.emit_state(app);
            }
        }
        AppCommand::SettingsMoveDown => {
            if matches!(app.view, crate::app::View::Settings) {
                app.settings_selected = (app.settings_selected + 1).min(SETTINGS_ITEMS_COUNT - 1);
                effects.emit_state(app);
            }
        }
        AppCommand::SettingsDecrease => {
            if matches!(app.view, crate::app::View::Settings) {
                let old_br = app.play_br;
                let old_crossfade = app.crossfade_ms;
                apply_settings_adjust(app, -1, next_song_cache);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                effects.send_audio_warn(
                    AudioCommand::SetVolume(app.volume),
                    "AudioWorker 通道已关闭：SetVolume 发送失败",
                );
                if old_br != app.play_br {
                    effects.send_audio(AudioCommand::SetCacheBr(app.play_br));
                }
                if old_crossfade != app.crossfade_ms {
                    effects.send_audio_warn(
                        AudioCommand::SetCrossfadeMs(app.crossfade_ms),
                        "AudioWorker 通道已关闭：SetCrossfadeMs 发送失败",
                    );
                }
                effects.emit_state(app);
            }
        }
        AppCommand::SettingsIncrease => {
            if matches!(app.view, crate::app::View::Settings) {
                let old_br = app.play_br;
                let old_crossfade = app.crossfade_ms;
                apply_settings_adjust(app, 1, next_song_cache);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                effects.send_audio_warn(
                    AudioCommand::SetVolume(app.volume),
                    "AudioWorker 通道已关闭：SetVolume 发送失败",
                );
                if old_br != app.play_br {
                    effects.send_audio(AudioCommand::SetCacheBr(app.play_br));
                }
                if old_crossfade != app.crossfade_ms {
                    effects.send_audio_warn(
                        AudioCommand::SetCrossfadeMs(app.crossfade_ms),
                        "AudioWorker 通道已关闭：SetCrossfadeMs 发送失败",
                    );
                }
                effects.emit_state(app);
            }
        }
        _ => return false,
    }
    true
}

/// 处理设置激活命令（SettingsActivate）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理
pub async fn handle_settings_activate_command(
    app: &mut App,
    effects: &mut CoreEffects,
) -> Option<bool> {
    if !matches!(app.view, crate::app::View::Settings) {
        return Some(false);
    }

    if is_clear_cache_selected(app) {
        app.settings_status = "正在清除音频缓存...".to_owned();
        tracing::info!("用户触发：清除音频缓存");
        effects.send_audio_warn(
            AudioCommand::ClearCache,
            "AudioWorker 通道已关闭：ClearCache 发送失败",
        );
        effects.emit_state(app);
        Some(true)
    } else if is_logout_selected(app) {
        if !app.logged_in {
            app.settings_status = "未登录，无需退出".to_owned();
            effects.emit_state(app);
            Some(true)
        } else {
            Some(false) // 由调用者处理登出逻辑
        }
    } else {
        Some(true)
    }
}

/// 处理播放器音量和模式控制命令（涉及设置持久化）
/// 返回 true 表示命令已处理，false 表示未处理
pub async fn handle_player_settings_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    effects: &mut CoreEffects,
    next_song_cache: &mut NextSongCacheManager,
) -> bool {
    match cmd {
        AppCommand::PlayerVolumeDown => {
            app.volume = (app.volume - 0.1).clamp(0.0, 2.0);
            effects.send_audio_warn(
                AudioCommand::SetVolume(app.volume),
                "AudioWorker 通道已关闭：SetVolume 发送失败",
            );
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            effects.emit_state(app);
        }
        AppCommand::PlayerVolumeUp => {
            app.volume = (app.volume + 0.1).clamp(0.0, 2.0);
            effects.send_audio_warn(
                AudioCommand::SetVolume(app.volume),
                "AudioWorker 通道已关闭：SetVolume 发送失败",
            );
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            effects.emit_state(app);
        }
        AppCommand::PlayerCycleMode => {
            app.play_mode = crate::features::player::playback::next_play_mode(app.play_mode);
            app.play_queue.set_mode(app.play_mode);
            app.play_status = format!(
                "播放模式: {}",
                crate::features::player::playback::play_mode_label(app.play_mode)
            );
            next_song_cache.reset(); // 失效预缓存
            sync_settings_from_app(settings, app);
            if let Err(e) = settings::save_settings(data_dir, settings) {
                tracing::warn!(err = %e, "保存设置失败");
            }
            effects.emit_state(app);
        }
        _ => return false,
    }
    true
}

/// 从设置同步到 App
pub fn apply_settings_to_app(app: &mut App, s: &settings::AppSettings) {
    app.volume = s.volume.clamp(0.0, 2.0);
    app.play_br = s.br;
    app.play_mode = settings::play_mode_from_string(&s.play_mode);
    app.play_queue.set_mode(app.play_mode);
    app.lyrics_offset_ms = s.lyrics_offset_ms;
    app.crossfade_ms = s.crossfade_ms;
}

/// 从 App 同步到设置
pub fn sync_settings_from_app(s: &mut settings::AppSettings, app: &App) {
    s.volume = app.volume;
    s.br = app.play_br;
    s.play_mode = settings::play_mode_to_string(app.play_mode);
    s.lyrics_offset_ms = app.lyrics_offset_ms;
    s.crossfade_ms = app.crossfade_ms;
}

fn is_logout_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 1
}

fn is_clear_cache_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 2
}

fn apply_settings_adjust(app: &mut App, dir: i32, next_song_cache: &mut NextSongCacheManager) {
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
                crate::features::player::playback::next_play_mode(app.play_mode)
            } else {
                crate::features::player::playback::prev_play_mode(app.play_mode)
            };
            app.play_queue.set_mode(app.play_mode);
            app.settings_status = format!(
                "播放模式: {}",
                crate::features::player::playback::play_mode_label(app.play_mode)
            );
            next_song_cache.reset(); // 失效预缓存
        }
        3 => {
            app.lyrics_offset_ms =
                app.lyrics_offset_ms
                    .saturating_add(if dir > 0 { 200 } else { -200 });
            app.settings_status = format!("歌词 offset: {}ms", app.lyrics_offset_ms);
        }
        4 => {
            let step = if dir > 0 { 50 } else { -50 };
            let next = (app.crossfade_ms as i64 + step).clamp(0, 2000) as u64;
            app.crossfade_ms = next;
            app.settings_status = if app.crossfade_ms == 0 {
                "淡入淡出已关闭".to_owned()
            } else {
                format!("淡入淡出: {}ms", app.crossfade_ms)
            };
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
