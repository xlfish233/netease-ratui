use crate::app::{PlaylistMode, View};
use std::time::Duration;

use crate::core::prelude::{
    app::App,
    audio::AudioCommand,
    effects::CoreEffects,
    infra::{NextSongCacheManager, RequestKey, RequestTracker},
    netease::NeteaseCommand,
};
use crate::core::utils;

pub fn next_play_mode(m: crate::app::PlayMode) -> crate::app::PlayMode {
    use crate::app::PlayMode;
    match m {
        PlayMode::Sequential => PlayMode::ListLoop,
        PlayMode::ListLoop => PlayMode::SingleLoop,
        PlayMode::SingleLoop => PlayMode::Shuffle,
        PlayMode::Shuffle => PlayMode::Sequential,
    }
}

pub fn prev_play_mode(m: crate::app::PlayMode) -> crate::app::PlayMode {
    use crate::app::PlayMode;
    match m {
        PlayMode::Sequential => PlayMode::Shuffle,
        PlayMode::ListLoop => PlayMode::Sequential,
        PlayMode::SingleLoop => PlayMode::ListLoop,
        PlayMode::Shuffle => PlayMode::SingleLoop,
    }
}

pub fn play_mode_label(m: crate::app::PlayMode) -> &'static str {
    use crate::app::PlayMode;
    match m {
        PlayMode::Sequential => "顺序",
        PlayMode::ListLoop => "列表循环",
        PlayMode::SingleLoop => "单曲循环",
        PlayMode::Shuffle => "随机",
    }
}

fn playback_elapsed_ms(app: &App) -> u64 {
    let Some(started) = app.play_started_at else {
        return 0;
    };

    let now = if app.paused {
        app.play_paused_at.unwrap_or_else(std::time::Instant::now)
    } else {
        std::time::Instant::now()
    };

    now.duration_since(started)
        .as_millis()
        .saturating_sub(app.play_paused_accum_ms as u128) as u64
}

pub fn seek_relative(app: &mut App, effects: &mut CoreEffects, delta_ms: i64) {
    let Some(total_ms) = app.play_total_ms else {
        return;
    };
    let cur = playback_elapsed_ms(app) as i64;
    let next = (cur + delta_ms).clamp(0, total_ms as i64) as u64;

    let now = std::time::Instant::now();
    app.play_started_at = Some(now - Duration::from_millis(next));
    if app.paused {
        app.play_paused_at = Some(now);
    } else {
        app.play_paused_at = None;
    }
    app.play_paused_accum_ms = 0;

    effects.send_audio(AudioCommand::SeekToMs(next));
}

pub(super) async fn request_play_at_index(
    app: &mut App,
    request_tracker: &mut RequestTracker<RequestKey>,
    song_request_titles: &mut std::collections::HashMap<i64, String>,
    req_id: &mut u64,
    idx: usize,
    next_song_cache: &mut NextSongCacheManager,
    effects: &mut CoreEffects,
) {
    if !app.play_queue.set_current_index(idx) {
        return;
    }
    let Some(s) = app.play_queue.songs().get(idx) else {
        return;
    };
    if matches!(app.view, View::Playlists) && matches!(app.playlist_mode, PlaylistMode::Tracks) {
        app.playlist_tracks_selected = idx.min(app.playlist_tracks.len().saturating_sub(1));
    }
    app.play_status = "获取播放链接...".to_owned();
    let title = format!("{} - {}", s.name, s.artists);
    song_request_titles.clear();
    let id = request_tracker.issue(RequestKey::SongUrl, || utils::next_id(req_id));
    song_request_titles.insert(s.id, title);
    effects.send_netease_hi(NeteaseCommand::SongUrl {
        req_id: id,
        id: s.id,
        br: app.play_br,
    });

    // 触发下一首预缓存
    next_song_cache.prefetch_next(app, effects, req_id).await;
}

pub async fn play_next(
    app: &mut App,
    request_tracker: &mut RequestTracker<RequestKey>,
    song_request_titles: &mut std::collections::HashMap<i64, String>,
    req_id: &mut u64,
    next_song_cache: &mut NextSongCacheManager,
    effects: &mut CoreEffects,
) {
    let Some(current_idx) = app.play_queue.current_index() else {
        return;
    };
    if app.play_queue.is_empty() {
        return;
    }

    let Some(peek_idx) = app.play_queue.peek_next_index() else {
        if matches!(app.play_mode, crate::app::PlayMode::Sequential) {
            app.play_status = "播放结束".to_owned();
            app.play_queue.clear_cursor();
        }
        return;
    };
    if peek_idx == current_idx && matches!(app.play_mode, crate::app::PlayMode::Sequential) {
        app.play_status = "播放结束".to_owned();
        app.play_queue.clear_cursor();
        return;
    }

    let Some(next_idx) = app.play_queue.next_index() else {
        return;
    };
    request_play_at_index(
        app,
        request_tracker,
        song_request_titles,
        req_id,
        next_idx,
        next_song_cache,
        effects,
    )
    .await;
}

pub(super) async fn play_prev(
    app: &mut App,
    request_tracker: &mut RequestTracker<RequestKey>,
    song_request_titles: &mut std::collections::HashMap<i64, String>,
    req_id: &mut u64,
    next_song_cache: &mut NextSongCacheManager,
    effects: &mut CoreEffects,
) {
    if app.play_queue.is_empty() || app.play_queue.current_index().is_none() {
        return;
    }
    let Some(prev_idx) = app.play_queue.prev_index() else {
        return;
    };
    request_play_at_index(
        app,
        request_tracker,
        song_request_titles,
        req_id,
        prev_idx,
        next_song_cache,
        effects,
    )
    .await;
}
