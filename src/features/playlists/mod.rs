use crate::app::{PlaylistMode, PlaylistPreload, PreloadStatus};

use crate::core::infra::{NextSongCacheManager, PreloadManager, RequestKey, RequestTracker};
use crate::core::prelude::{
    app::App, effects::CoreEffects, messages::AppCommand, netease::NeteaseCommand,
};
use crate::core::utils;

mod tracks;

pub use tracks::PlaylistTracksLoad;

/// 分页大小：PageDown/PageUp 一次跳转的行数
const PAGE_SIZE: usize = 10;

/// 处理歌单相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
#[allow(clippy::too_many_arguments)]
pub async fn handle_playlists_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    request_tracker: &mut RequestTracker<RequestKey>,
    song_request_titles: &mut std::collections::HashMap<i64, String>,
    playlist_tracks_loader: &mut Option<PlaylistTracksLoad>,
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
        AppCommand::PlaylistsMoveTo { index } => {
            if !app.playlists.is_empty() && index < app.playlists.len() {
                app.playlists_selected = index;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistsPageDown => {
            if !app.playlists.is_empty() {
                let new_idx =
                    (app.playlists_selected + PAGE_SIZE).min(app.playlists.len().saturating_sub(1));
                if new_idx != app.playlists_selected {
                    app.playlists_selected = new_idx;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistsPageUp => {
            if !app.playlists.is_empty() {
                let new_idx = app.playlists_selected.saturating_sub(PAGE_SIZE);
                if new_idx != app.playlists_selected {
                    app.playlists_selected = new_idx;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistsJumpTop => {
            if !app.playlists.is_empty() && app.playlists_selected != 0 {
                app.playlists_selected = 0;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistsJumpBottom => {
            if !app.playlists.is_empty() {
                let last = app.playlists.len().saturating_sub(1);
                if app.playlists_selected != last {
                    app.playlists_selected = last;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistsOpenSelected => {
            if matches!(app.playlist_mode, PlaylistMode::List) {
                let Some(playlist_id) = app.playlists.get(app.playlists_selected).map(|p| p.id)
                else {
                    return true;
                };

                // 新增：检查前的日志
                tracing::info!(
                    "🎵 [Playlists] 打开歌单: playlist_id={}, playlist_preloads.contains_key={}",
                    playlist_id,
                    app.playlist_preloads.contains_key(&playlist_id)
                );

                // 检查是否已有预加载完成的歌曲
                if let std::collections::hash_map::Entry::Occupied(mut entry) =
                    app.playlist_preloads.entry(playlist_id)
                {
                    let preload = entry.get_mut();
                    tracing::info!(
                        "🎵 [Playlists] 预加载状态: status={:?}, songs={}",
                        preload.status,
                        preload.songs.len()
                    );

                    if matches!(preload.status, PreloadStatus::Completed)
                        && !preload.songs.is_empty()
                    {
                        // 保留 playlist_tracks 给 UI 显示，同时克隆给 play_queue
                        app.playlist_tracks = preload.songs.clone();
                        app.playlist_tracks_selected = 0;
                        app.playlist_mode = PlaylistMode::Tracks;

                        // 克隆一份给 play_queue（不转移 playlist_tracks 的所有权）
                        let _old = app.play_queue.set_songs(preload.songs.clone(), Some(0));

                        next_song_cache.reset(); // 失效预缓存
                        app.playlists_status =
                            format!("歌曲: {} 首（已缓存，p 播放）", app.playlist_tracks.len());
                        // 新增：使用预加载的日志
                        tracing::info!(
                            "🎵 [Playlists] 使用预加载数据: playlist_id={}, songs={}",
                            playlist_id,
                            app.playlist_tracks.len()
                        );
                        effects.emit_state(app);
                        return true;
                    }
                }

                // 新增：没有可用预加载的日志
                tracing::info!(
                    "🎵 [Playlists] 无可用预加载，发起网络请求: playlist_id={}",
                    playlist_id
                );

                // 用户主动打开歌单：取消该歌单的预加载（若正在进行），并走高优先级加载
                preload_mgr.cancel_playlist(app, playlist_id);

                app.playlists_status = "加载歌单歌曲中...".to_owned();
                *playlist_tracks_loader = None;
                effects.emit_state(app);
                let id =
                    request_tracker.issue(RequestKey::PlaylistDetail, || utils::next_id(req_id));
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
        AppCommand::PlaylistTracksMoveTo { index } => {
            if !app.playlist_tracks.is_empty() && index < app.playlist_tracks.len() {
                app.playlist_tracks_selected = index;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistTracksPageDown => {
            if !app.playlist_tracks.is_empty() {
                let new_idx = (app.playlist_tracks_selected + PAGE_SIZE)
                    .min(app.playlist_tracks.len().saturating_sub(1));
                if new_idx != app.playlist_tracks_selected {
                    app.playlist_tracks_selected = new_idx;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistTracksPageUp => {
            if !app.playlist_tracks.is_empty() {
                let new_idx = app.playlist_tracks_selected.saturating_sub(PAGE_SIZE);
                if new_idx != app.playlist_tracks_selected {
                    app.playlist_tracks_selected = new_idx;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistTracksJumpTop => {
            if !app.playlist_tracks.is_empty() && app.playlist_tracks_selected != 0 {
                app.playlist_tracks_selected = 0;
                effects.emit_state(app);
            }
        }
        AppCommand::PlaylistTracksJumpBottom => {
            if !app.playlist_tracks.is_empty() {
                let last = app.playlist_tracks.len().saturating_sub(1);
                if app.playlist_tracks_selected != last {
                    app.playlist_tracks_selected = last;
                    effects.emit_state(app);
                }
            }
        }
        AppCommand::PlaylistTracksPlaySelected => {
            if matches!(app.playlist_mode, PlaylistMode::Tracks)
                && let Some(s) = app.playlist_tracks.get(app.playlist_tracks_selected)
            {
                app.play_status = "获取播放链接...".to_owned();

                // 先保存歌曲信息，因为后续会转移所有权
                let song_id = s.id;
                let title = format!("{} - {}", s.name, s.artists);

                // 克隆一份给 play_queue（保留 playlist_tracks 给 UI 显示）
                let _old = app.play_queue.set_songs(
                    app.playlist_tracks.clone(),
                    Some(app.playlist_tracks_selected),
                );

                next_song_cache.reset(); // 失效预缓存
                effects.emit_state(app);
                song_request_titles.clear();
                let id = request_tracker.issue(RequestKey::SongUrl, || utils::next_id(req_id));
                song_request_titles.insert(song_id, title);
                effects.send_netease_hi(NeteaseCommand::SongUrl {
                    req_id: id,
                    id: song_id,
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
    playlist_tracks_loader: &mut Option<PlaylistTracksLoad>,
    effects: &mut CoreEffects,
) -> bool {
    if matches!(cmd, AppCommand::Back) && matches!(app.view, crate::app::View::Playlists) {
        app.playlist_mode = PlaylistMode::List;
        *playlist_tracks_loader = None;
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
    request_tracker: &mut RequestTracker<RequestKey>,
    preload_mgr: &mut PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
    preload_count: usize,
) -> bool {
    let key = RequestKey::Playlists;
    if !request_tracker.accept(&key, req_id) {
        return false;
    }
    app.playlists = playlists;
    app.playlists_selected = app
        .playlists
        .iter()
        .position(|p| p.special_type == 5 || p.name.contains("我喜欢"))
        .unwrap_or(0);
    app.playlist_mode = PlaylistMode::List;
    app.playlist_tracks.clear();
    app.playlist_tracks_selected = 0;

    // 新增：在调用 start_for_playlists 前记录
    tracing::info!(
        "🎵 [Playlists] 收到歌单列表, 准备调用 start_for_playlists, 当前 playlist_preloads count={}",
        app.playlist_preloads.len()
    );

    preload_mgr
        .start_for_playlists(app, effects, next_req_id, preload_count)
        .await;

    // 新增：调用后记录
    tracing::info!(
        "🎵 [Playlists] start_for_playlists 完成, playlist_preloads count={}",
        app.playlist_preloads.len()
    );

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
    request_tracker: &mut RequestTracker<RequestKey>,
    playlist_tracks_loader: &mut Option<PlaylistTracksLoad>,
    preload_mgr: &PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
) -> Option<bool> {
    // 检查是否是预加载管理器的请求
    if preload_mgr.owns_req(req_id) {
        return Some(false); // 由预加载管理器处理
    }

    let key = RequestKey::PlaylistDetail;
    if !request_tracker.accept(&key, req_id) {
        return None;
    }
    if ids.is_empty() {
        app.playlists_status = "歌单为空或无法解析".to_owned();
        effects.emit_state(app);
        return Some(true);
    }

    app.playlists_status = format!("加载歌单歌曲中... 0/{}", ids.len());
    effects.emit_state(app);

    let mut loader = PlaylistTracksLoad::new(playlist_id, ids);
    let id = request_tracker.issue(RequestKey::PlaylistTracks, || utils::next_id(next_req_id));
    let chunk = loader.next_chunk();
    loader.inflight_req_id = Some(id);
    *playlist_tracks_loader = Some(loader);
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
    request_tracker: &mut RequestTracker<RequestKey>,
    playlist_tracks_loader: &mut Option<PlaylistTracksLoad>,
    preload_mgr: &mut PreloadManager,
    effects: &mut CoreEffects,
    next_req_id: &mut u64,
) -> Option<bool> {
    // 检查是否是预加载管理器的请求
    if preload_mgr.owns_req(req_id) {
        return Some(false); // 由预加载管理器处理
    }

    let Some(loader) = playlist_tracks_loader.as_mut() else {
        return Some(false);
    };
    if loader.inflight_req_id != Some(req_id) {
        return Some(false);
    }
    if !request_tracker.accept(&RequestKey::PlaylistTracks, req_id) {
        return Some(false);
    }
    loader.inflight_req_id = None;
    loader.songs.extend(songs);

    app.playlists_status = format!("加载歌单歌曲中... {}/{}", loader.songs.len(), loader.total);
    effects.emit_state(app);

    if loader.is_done() {
        let Some(loader) = playlist_tracks_loader.take() else {
            tracing::warn!("pending_playlist_tracks 丢失（已完成但无法 take）");
            return Some(true);
        };
        let playlist_id = loader.playlist_id;
        let songs = loader.songs;

        // 更新预加载缓存
        // 克隆保存到预加载缓存（songs 会被赋值给 playlist_tracks）
        // 注意：这里需要克隆是因为 songs 还要赋值给 app.playlist_tracks
        // TODO: 考虑使用 Arc<Song> 避免克隆
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

        app.playlist_tracks = songs.clone();
        app.playlist_tracks_selected = 0;
        app.playlist_mode = PlaylistMode::Tracks;

        // 克隆一份给 play_queue（保留 playlist_tracks 给 UI 显示）
        let _old = app.play_queue.set_songs(songs, Some(0));

        app.playlists_status = format!("歌曲: {} 首（p 播放）", app.playlist_tracks.len());
        effects.emit_state(app);
        Some(true)
    } else {
        let id = request_tracker.issue(RequestKey::PlaylistTracks, || utils::next_id(next_req_id));
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
        // 计算普通歌单数量（排除"我喜欢的音乐"）
        let normal_count = app.playlists.iter().filter(|p| p.special_type != 5).count();

        let mut s = format!("歌单[{}]（已选中我喜欢的音乐，回车打开）", normal_count);
        if !app.preload_summary.is_empty() {
            s.push_str(" | ");
            s.push_str(&app.preload_summary);
        }
        app.playlists_status = s;
    }
}
