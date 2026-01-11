use crate::app::{App, PlaylistMode, PlaylistPreload, PreloadStatus};
use crate::audio_worker::AudioCommand;
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::actor::NeteaseCommand;
use crate::usecases::actor::playlist_tracks;
use crate::usecases::actor::preload::PreloadManager;

use super::utils;
use tokio::sync::mpsc;

/// 处理歌单相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_playlists_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    pending_song_url: &mut Option<(u64, String)>,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<playlist_tracks::PlaylistTracksLoad>,
    preload_mgr: &mut PreloadManager,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    _tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    match cmd {
        AppCommand::PlaylistsMoveUp => {
            if app.playlists_selected > 0 {
                app.playlists_selected -= 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::PlaylistsMoveDown => {
            if !app.playlists.is_empty() && app.playlists_selected + 1 < app.playlists.len() {
                app.playlists_selected += 1;
                utils::push_state(tx_evt, app).await;
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
                    app.playlists_status =
                        format!("歌曲: {} 首（已缓存，p 播放）", app.playlist_tracks.len());
                    utils::push_state(tx_evt, app).await;
                    return true;
                }

                // 用户主动打开歌单：取消该歌单的预加载（若正在进行），并走高优先级加载
                preload_mgr.cancel_playlist(app, playlist_id);

                app.playlists_status = "加载歌单歌曲中...".to_owned();
                *pending_playlist_tracks = None;
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_playlist_detail = Some(id);
                let _ = tx_netease_hi
                    .send(NeteaseCommand::PlaylistDetail {
                        req_id: id,
                        playlist_id,
                    })
                    .await;
            }
        }
        AppCommand::PlaylistTracksMoveUp => {
            if app.playlist_tracks_selected > 0 {
                app.playlist_tracks_selected -= 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::PlaylistTracksMoveDown => {
            if !app.playlist_tracks.is_empty()
                && app.playlist_tracks_selected + 1 < app.playlist_tracks.len()
            {
                app.playlist_tracks_selected += 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::PlaylistTracksPlaySelected => {
            if matches!(app.playlist_mode, PlaylistMode::Tracks)
                && let Some(s) = app.playlist_tracks.get(app.playlist_tracks_selected)
            {
                app.play_status = "获取播放链接...".to_owned();
                app.queue = app.playlist_tracks.clone();
                app.queue_pos = Some(app.playlist_tracks_selected);
                let title = format!("{} - {}", s.name, s.artists);
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_song_url = Some((id, title));
                let _ = tx_netease_hi
                    .send(NeteaseCommand::SongUrl {
                        req_id: id,
                        id: s.id,
                        br: app.play_br,
                    })
                    .await;
            }
        }
        _ => return false,
    }
    true
}

/// 处理歌单列表 Back 命令
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_playlists_back_command(
    cmd: AppCommand,
    app: &mut App,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<playlist_tracks::PlaylistTracksLoad>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    if matches!(cmd, AppCommand::Back) && matches!(app.view, crate::app::View::Playlists) {
        app.playlist_mode = PlaylistMode::List;
        *pending_playlist_detail = None;
        *pending_playlist_tracks = None;
        refresh_playlist_list_status(app);
        utils::push_state(tx_evt, app).await;
        return true;
    }
    false
}

/// 处理歌单相关的 NeteaseEvent::Playlists
/// 返回 true 表示事件已处理，false 表示 req_id 不匹配
pub(super) async fn handle_playlists_event(
    req_id: u64,
    playlists: Vec<crate::domain::model::Playlist>,
    app: &mut App,
    pending_playlists: &mut Option<u64>,
    preload_mgr: &mut PreloadManager,
    tx_netease_lo: &mpsc::Sender<NeteaseCommand>,
    next_req_id: &mut u64,
    tx_evt: &mpsc::Sender<AppEvent>,
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
        .start_for_playlists(app, tx_netease_lo, next_req_id)
        .await;

    refresh_playlist_list_status(app);
    utils::push_state(tx_evt, app).await;
    true
}

/// 处理歌单详情相关的事件（PlaylistTrackIds）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理，None 表示 req_id 不匹配
pub(super) async fn handle_playlist_detail_event(
    req_id: u64,
    playlist_id: i64,
    ids: Vec<i64>,
    app: &mut App,
    pending_playlist_detail: &mut Option<u64>,
    pending_playlist_tracks: &mut Option<playlist_tracks::PlaylistTracksLoad>,
    preload_mgr: &PreloadManager,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    next_req_id: &mut u64,
    tx_evt: &mpsc::Sender<AppEvent>,
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
        utils::push_state(tx_evt, app).await;
        return Some(true);
    }

    app.playlists_status = format!("加载歌单歌曲中... 0/{}", ids.len());
    utils::push_state(tx_evt, app).await;

    let mut loader = playlist_tracks::PlaylistTracksLoad::new(playlist_id, ids);
    let id = utils::next_id(next_req_id);
    let chunk = loader.next_chunk();
    loader.inflight_req_id = Some(id);
    *pending_playlist_tracks = Some(loader);
    let _ = tx_netease_hi
        .send(NeteaseCommand::SongDetailByIds {
            req_id: id,
            ids: chunk,
        })
        .await;
    Some(true)
}

/// 处理歌单歌曲批量加载的事件（Songs）
/// 返回 Some(true) 表示已处理且应 continue，Some(false) 表示未处理
pub(super) async fn handle_songs_event(
    req_id: u64,
    songs: Vec<crate::domain::model::Song>,
    app: &mut App,
    pending_playlist_tracks: &mut Option<playlist_tracks::PlaylistTracksLoad>,
    preload_mgr: &mut PreloadManager,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    next_req_id: &mut u64,
    tx_evt: &mpsc::Sender<AppEvent>,
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
    utils::push_state(tx_evt, app).await;

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
            use crate::usecases::actor::preload;
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
        utils::push_state(tx_evt, app).await;
        Some(true)
    } else {
        let id = utils::next_id(next_req_id);
        let chunk = loader.next_chunk();
        loader.inflight_req_id = Some(id);
        let _ = tx_netease_hi
            .send(NeteaseCommand::SongDetailByIds {
                req_id: id,
                ids: chunk,
            })
            .await;
        Some(true)
    }
}

/// 刷新歌单列表状态文本
pub(super) fn refresh_playlist_list_status(app: &mut App) {
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
