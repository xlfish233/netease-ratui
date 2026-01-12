use crate::app::{PlaylistMode, PlaylistPreload, PreloadStatus};

use crate::core::infra::{NextSongCacheManager, PreloadManager};
use crate::core::prelude::{
    app::App, effects::CoreEffects, messages::AppCommand, netease::NeteaseCommand,
};
use crate::core::utils;

mod tracks;

pub use tracks::PlaylistTracksLoad;

/// 处理歌单相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
#[allow(clippy::too_many_arguments)]
pub async fn handle_playlists_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    pending_song_url: &mut Option<(u64, String)>,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<PlaylistTracksLoad>,
    preload_mgr: &mut PreloadManager,
    effects: &mut CoreEffects,
    next_song_cache: &mut NextSongCacheManager,
) -> bool {
    match cmd {
        AppCommand::PlaylistsMoveUp => {
            if app.playlists_selected > 0 {
                app.playlists_selected -= 1;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistsMoveDown => {
            if !app.playlists.is_empty() && app.playlists_selected + 1 < app.playlists.len() {
                app.playlists_selected += 1;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistsOpenSelected => {
            if matches!(app.playlist_mode, PlaylistMode::List) {
                let Some(playlist_id) = app.playlists.get(app.playlists_selected).map(|p| p.id)
                else {
                    return true;
                };

                // 检查是否已有预加载完成的歌曲
                if let Some(preload) = app.playlist_preloads.get(&playlist_id)
                    && matches!(preload.status, PreloadStatus::Completed)
                    && !preload.songs.is_empty()
                {
                    app.playlist_tracks = preload.songs.clone();
                    app.playlist_tracks_selected = 0;
                    app.playlist_mode = PlaylistMode::Tracks;
                    app.queue = app.playlist_tracks.clone();
                    app.queue_pos = Some(0);
                    next_song_cache.reset(); // 失效预缓存
                    app.playlists_status =
                        format!("歌曲: {} 首（已缓存，p 播放）", app.playlist_tracks.len());
                    effects.emit_state(app);
                    return true;
                }

                // 用户主动打开歌单：取消该歌单的预加载（若正在进行），并走高优先级加载
                preload_mgr.cancel_playlist(app, playlist_id);

                app.playlists_status = "加载歌单歌曲中...".to_owned();
                *pending_playlist_tracks = None;
                effects.emit_state(app);
                let id = utils::next_id(req_id);
                *pending_playlist_detail = Some(id);
                effects.send_netease_hi(NeteaseCommand::PlaylistDetail {
                    req_id: id,
                    playlist_id,
                });
            }
        }
        AppCommand::PlaylistTracksMoveUp => {
            if app.playlist_tracks_selected > 0 {
                app.playlist_tracks_selected -= 1;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistTracksMoveDown => {
            if !app.playlist_tracks.is_empty()
                && app.playlist_tracks_selected + 1 < app.playlist_tracks.len()
            {
                app.playlist_tracks_selected += 1;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistTracksPlaySelected => {
            if matches!(app.playlist_mode, PlaylistMode::Tracks)
                && let Some(s) = app.playlist_tracks.get(app.playlist_tracks_selected)
            {
                app.play_status = "获取播放链接...".to_owned();
                app.queue = app.playlist_tracks.clone();
                app.queue_pos = Some(app.playlist_tracks_selected);
                next_song_cache.reset(); // 失效预缓存
                let title = format!("{} - {}", s.name, s.artists);
                effects.emit_state(app);
                let id = utils::next_id(req_id);
                *pending_song_url = Some((id, title));
                effects.send_netease_hi(NeteaseCommand::SongUrl {
                    req_id: id,
                    id: s.id,
                    br: app.play_br,
                });
            }
        }
        _ => return false,
    }
    true
}

/// 处理歌单列表 Back 命令
/// 返回 true 表示命令已处理，false 表示未处理
pub async fn handle_playlists_back_command(
    cmd: AppCommand,
    app: &mut App,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<PlaylistTracksLoad>,
    effects: &mut CoreEffects,
) -> bool {
    if matches!(cmd, AppCommand::Back) && matches!(app.view, crate::app::View::Playlists) {
        app.playlist_mode = PlaylistMode::List;
        *pending_playlist_detail = None;
        *pending_playlist_tracks = None;
        refresh_playlist_list_status(app);
        effects.emit_state(app);
        return true;
    }
    false
}

/// 处理歌单相关的 NeteaseEvent::Playlists
/// 返回 true 表示事件已处理，false 表示 req_id 不匹配
#[allow(clippy::too_many_arguments)]
pub async fn handle_playlists_event(
    req_id: u64,
    playlists: Vec<crate::domain::model::Playlist>,
    app: &mut App,
    pending_playlists: &mut Option<u64>,
    preload_mgr: &mut PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
    preload_count: usize,
) -> bool {
    if *pending_playlists != Some(req_id) {
        return false;
    }
    *pending_playlists = None;
    app.playlists = playlists;
    app.playlists_selected = app
        .playlists
        .iter()
        .position(|p| p.special_type == 5 || p.name.contains("我喜欢"))
        .unwrap_or(0);
    app.playlist_mode = PlaylistMode::List;
    app.playlist_tracks.clear();
    app.playlist_tracks_selected = 0;

    preload_mgr
        .start_for_playlists(app, effects, next_req_id, preload_count)
        .await;

    refresh_playlist_list_status(app);
    effects.emit_state(app);
    true
}

/// 处理歌单详情相关的事件（PlaylistTrackIds）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理，None 表示 req_id 不匹配
#[allow(clippy::too_many_arguments)]
pub async fn handle_playlist_detail_event(
    req_id: u64,
    playlist_id: i64,
    ids: Vec<i64>,
    app: &mut App,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<PlaylistTracksLoad>,
    preload_mgr: &PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
) -> Option<bool> {
    // 检查是否是预加载管理器的请求
    if preload_mgr.owns_req(req_id) {
        return Some(false); // 由预加载管理器处理
    }

    if *pending_playlist_detail != Some(req_id) {
        return None;
    }
    *pending_playlist_detail = None;
    if ids.is_empty() {
        app.playlists_status = "歌单为空或无法解析".to_owned();
        effects.emit_state(app);
        return Some(true);
    }

    app.playlists_status = format!("加载歌单歌曲中... 0/{}", ids.len());
    effects.emit_state(app);

    let mut loader = PlaylistTracksLoad::new(playlist_id, ids);
    let id = utils::next_id(next_req_id);
    let chunk = loader.next_chunk();
    loader.inflight_req_id = Some(id);
    *pending_playlist_tracks = Some(loader);
    effects.send_netease_hi(NeteaseCommand::SongDetailByIds {
        req_id: id,
        ids: chunk,
    });
    Some(true)
}

/// 处理歌单歌曲批量加载的事件（Songs）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理
#[allow(clippy::too_many_arguments)]
pub async fn handle_songs_event(
    req_id: u64,
    songs: Vec<crate::domain::model::Song>,
    app: &mut App,
    pending_playlist_tracks: &mut Option<PlaylistTracksLoad>,
    preload_mgr: &mut PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
) -> Option<bool> {
    // 检查是否是预加载管理器的请求
    if preload_mgr.owns_req(req_id) {
        return Some(false); // 由预加载管理器处理
    }

    let Some(loader) = pending_playlist_tracks.as_mut() else {
        return Some(false);
    };
    if loader.inflight_req_id != Some(req_id) {
        return Some(false);
    }
    loader.inflight_req_id = None;
    loader.songs.extend(songs);

    app.playlists_status = format!("加载歌单歌曲中... {}/{}", loader.songs.len(), loader.total);
    effects.emit_state(app);

    if loader.is_done() {
        let Some(loader) = pending_playlist_tracks.take() else {
            tracing::warn!("pending_playlist_tracks 丢失（已完成但无法 take）");
            return Some(true);
        };
        let playlist_id = loader.playlist_id;
        let songs = loader.songs;

        // 更新预加载缓存
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            app.playlist_preloads.entry(playlist_id)
        {
            use crate::core::infra::preload_pub as preload;
            entry.insert(PlaylistPreload {
                status: PreloadStatus::Completed,
                songs: songs.clone(),
            });
            preload::update_preload_summary(app);
        }

        app.playlist_tracks = songs;
        app.playlist_tracks_selected = 0;
        app.playlist_mode = PlaylistMode::Tracks;
        app.queue = app.playlist_tracks.clone();
        app.queue_pos = Some(0);
        app.playlists_status = format!("歌曲: {} 首（p 播放）", app.playlist_tracks.len());
        effects.emit_state(app);
        Some(true)
    } else {
        let id = utils::next_id(next_req_id);
        let chunk = loader.next_chunk();
        loader.inflight_req_id = Some(id);
        effects.send_netease_hi(NeteaseCommand::SongDetailByIds {
            req_id: id,
            ids: chunk,
        });
        Some(true)
    }
}

/// 刷新歌单列表状态文本
pub fn refresh_playlist_list_status(app: &mut App) {
    if matches!(app.view, crate::app::View::Playlists)
        && matches!(app.playlist_mode, PlaylistMode::List)
    {
        let mut s = format!(
            "歌单: {} 个（已选中我喜欢的音乐，回车打开）",
            app.playlists.len()
        );
        if !app.preload_summary.is_empty() {
            s.push_str(" | ");
            s.push_str(&app.preload_summary);
        }
        app.playlists_status = s;
    }
}
