use crate::app::{PlaylistMode, View};
use rand::Rng;
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

/// 计算下一首的索引（仅计算，不触发播放）
/// 返回 Some(idx) 表示有下一首，None 表示无下一首（如 Sequential 到末尾）
pub fn calculate_next_index(app: &App) -> Option<usize> {
    use crate::app::PlayMode;

    let pos = app.queue_pos?;
    if app.queue.is_empty() {
        return None;
    }

    match app.play_mode {
        PlayMode::SingleLoop => Some(pos), // 下一首是当前
        PlayMode::Shuffle => None,         // 随机模式不预测
        PlayMode::Sequential => {
            let n = pos + 1;
            if n >= app.queue.len() {
                None // 到末尾，无下一首
            } else {
                Some(n)
            }
        }
        PlayMode::ListLoop => Some((pos + 1) % app.queue.len()),
    }
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
    let Some(s) = app.queue.get(idx) else {
        return;
    };
    app.queue_pos = Some(idx);
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
    use crate::app::PlayMode;

    let Some(pos) = app.queue_pos else {
        return;
    };
    if app.queue.is_empty() {
        return;
    }

    let next_idx = match app.play_mode {
        PlayMode::SingleLoop => pos,
        PlayMode::Shuffle => {
            if app.queue.len() == 1 {
                pos
            } else {
                let mut rng = rand::thread_rng();
                loop {
                    let idx = rng.gen_range(0..app.queue.len());
                    if idx != pos {
                        break idx;
                    }
                }
            }
        }
        PlayMode::Sequential => {
            let n = pos + 1;
            if n >= app.queue.len() {
                app.play_status = "播放结束".to_owned();
                app.queue_pos = None;
                return;
            }
            n
        }
        PlayMode::ListLoop => (pos + 1) % app.queue.len(),
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
    use crate::app::PlayMode;

    let Some(pos) = app.queue_pos else {
        return;
    };
    if app.queue.is_empty() {
        return;
    }

    let prev_idx = match app.play_mode {
        PlayMode::SingleLoop => pos,
        PlayMode::Shuffle => {
            if app.queue.len() == 1 {
                pos
            } else {
                let mut rng = rand::thread_rng();
                loop {
                    let idx = rng.gen_range(0..app.queue.len());
                    if idx != pos {
                        break idx;
                    }
                }
            }
        }
        PlayMode::Sequential => pos.saturating_sub(1),
        PlayMode::ListLoop => {
            if pos == 0 {
                app.queue.len() - 1
            } else {
                pos - 1
            }
        }
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
