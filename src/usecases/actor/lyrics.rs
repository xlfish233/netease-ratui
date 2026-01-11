use crate::app::App;
use crate::messages::app::{AppCommand, AppEvent};
use crate::settings;

use super::utils;
use tokio::sync::mpsc;

/// 处理歌词相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_lyrics_command(
    cmd: AppCommand,
    app: &mut App,
    settings: &mut settings::AppSettings,
    data_dir: &std::path::Path,
    tx_evt: &mpsc::Sender<AppEvent>,
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
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::LyricsMoveUp => {
            if matches!(app.view, crate::app::View::Lyrics)
                && !app.lyrics_follow
                && app.lyrics_selected > 0
            {
                app.lyrics_selected -= 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::LyricsMoveDown => {
            if matches!(app.view, crate::app::View::Lyrics)
                && !app.lyrics_follow
                && !app.lyrics.is_empty()
                && app.lyrics_selected + 1 < app.lyrics.len()
            {
                app.lyrics_selected += 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::LyricsGotoCurrent => {
            if matches!(app.view, crate::app::View::Lyrics) {
                app.lyrics_follow = true;
                app.lyrics_status = "歌词：跟随模式".to_owned();
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::LyricsOffsetAddMs { ms } => {
            if matches!(app.view, crate::app::View::Lyrics) {
                app.lyrics_offset_ms = app.lyrics_offset_ms.saturating_add(ms);
                sync_settings_from_app(settings, app);
                if let Err(e) = settings::save_settings(data_dir, settings) {
                    tracing::warn!(err = %e, "保存设置失败");
                }
                utils::push_state(tx_evt, app).await;
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
pub(super) async fn handle_lyric_event(
    req_id: u64,
    song_id: i64,
    lyrics: Vec<crate::domain::model::LyricLine>,
    app: &mut App,
    pending_lyric: &mut Option<(u64, i64)>,
    tx_evt: &mpsc::Sender<AppEvent>,
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
    utils::push_state(tx_evt, app).await;
    true
}

/// 从 App 同步歌词 offset 到设置
pub(super) fn sync_settings_from_app(settings: &mut settings::AppSettings, app: &App) {
    settings.lyrics_offset_ms = app.lyrics_offset_ms;
}
