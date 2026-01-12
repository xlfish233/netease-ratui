use crate::core::prelude::{app::App, effects::CoreEffects, messages::AppCommand};
use crate::settings;

/// 处理歌词相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub async fn handle_lyrics_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    effects: &mut CoreEffects,
) -> bool {
    match cmd {
        AppCommand::LyricsToggleFollow => {
            if matches!(app.view, crate::app::View::Lyrics) {
                app.lyrics_follow = !app.lyrics_follow;
                if app.lyrics_follow {
                    app.lyrics_status = "歌词：跟随模式".to_owned();
                } else {
                    app.lyrics_status = "歌词：锁定模式（↑↓滚动，g 回到当前行）".to_owned();
                }
                effects.emit_state(app);
            }
        }
        AppCommand::LyricsMoveUp => {
            if matches!(app.view, crate::app::View::Lyrics)
                && !app.lyrics_follow
                && app.lyrics_selected > 0
            {
                app.lyrics_selected -= 1;
                effects.emit_state(app);
            }
        }
        AppCommand::LyricsMoveDown => {
            if matches!(app.view, crate::app::View::Lyrics)
                && !app.lyrics_follow
                && !app.lyrics.is_empty()
                && app.lyrics_selected + 1 < app.lyrics.len()
            {
                app.lyrics_selected += 1;
                effects.emit_state(app);
            }
        }
        AppCommand::LyricsGotoCurrent => {
            if matches!(app.view, crate::app::View::Lyrics) {
                app.lyrics_follow = true;
                app.lyrics_status = "歌词：跟随模式".to_owned();
                effects.emit_state(app);
            }
        }
        AppCommand::LyricsOffsetAddMs { ms } => {
            if matches!(app.view, crate::app::View::Lyrics) {
                app.lyrics_offset_ms = app.lyrics_offset_ms.saturating_add(ms);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                effects.emit_state(app);
            }
        }
        _ => return false,
    }
    true
}

/// 处理歌词相关的 NeteaseEvent::Lyric
/// req_id: 请求ID，用于匹配pending请求
/// song_id: 歌曲ID
/// lyrics: 歌词列表
/// 返回 true 表示事件已处理，false 表示 req_id 不匹配
pub async fn handle_lyric_event(
    req_id: u64,
    song_id: i64,
    lyrics: Vec<crate::domain::model::LyricLine>,
    app: &mut App,
    pending_lyric: &mut Option<(u64, i64)>,
    effects: &mut CoreEffects,
) -> bool {
    if pending_lyric.map(|(rid, _)| rid) != Some(req_id) {
        return false;
    }
    *pending_lyric = None;
    app.lyrics_song_id = Some(song_id);
    app.lyrics = lyrics;
    app.lyrics_selected = 0;
    app.lyrics_status = if app.lyrics.is_empty() {
        "暂无歌词".to_owned()
    } else {
        format!("歌词: {} 行", app.lyrics.len())
    };
    effects.emit_state(app);
    true
}

/// 从 App 同步歌词 offset 到设置
pub fn sync_settings_from_app(settings: &mut settings::AppSettings, app: &App) {
    settings.lyrics_offset_ms = app.lyrics_offset_ms;
}
