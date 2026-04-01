use crate::core::prelude::{
    app::App,
    audio::{
        AudioBufferState, AudioCommand, AudioEvent, AudioLoadStage, AudioPlaybackMode,
        AudioStreamHint,
    },
    effects::CoreEffects,
    infra::{NextSongCacheManager, RequestKey, RequestTracker},
    netease::NeteaseCommand,
};
use crate::core::utils;
use crate::features::player::playback::play_next;
use std::time::{Duration, Instant};

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn format_download_progress(downloaded_bytes: u64, total_bytes: Option<u64>) -> String {
    match total_bytes.filter(|total| *total > 0) {
        Some(total_bytes) => {
            let percent = downloaded_bytes.saturating_mul(100) / total_bytes;
            format!(
                "{percent}% ({}/{})",
                format_bytes(downloaded_bytes),
                format_bytes(total_bytes)
            )
        }
        None => format_bytes(downloaded_bytes),
    }
}

fn format_playback_status(paused: bool, stream_hint: Option<&AudioStreamHint>) -> String {
    let base = if paused { "已暂停" } else { "播放中" };
    let Some(hint) = stream_hint else {
        return base.to_owned();
    };

    match hint.mode {
        AudioPlaybackMode::CachedFile => base.to_owned(),
        AudioPlaybackMode::ProgressiveStream if hint.seekable => {
            format!("{base}（已缓存完成，可拖动）")
        }
        AudioPlaybackMode::ProgressiveStream => match hint.buffer_state {
            AudioBufferState::Prebuffering => format!("{base}（预缓冲中）"),
            AudioBufferState::Buffering | AudioBufferState::Ready => {
                format!("{base}（边下边播，暂不可拖动）")
            }
            AudioBufferState::Stalled => format!("{base}（缓冲不足，等待数据）"),
        },
    }
}

fn format_loading_status(
    title: &str,
    stage: &AudioLoadStage,
    stream_hint: Option<&AudioStreamHint>,
    is_currently_playing: bool,
) -> String {
    if let Some(hint) = stream_hint
        && matches!(hint.mode, AudioPlaybackMode::ProgressiveStream)
    {
        return match stage {
            AudioLoadStage::CacheHit => format!("缓存命中，准备播放: {title}"),
            AudioLoadStage::DownloadQueued => {
                if is_currently_playing {
                    format!("播放中，等待更多流式数据: {title}")
                } else {
                    format!("缓存未命中，开始预缓冲: {title}")
                }
            }
            AudioLoadStage::Downloading {
                downloaded_bytes,
                total_bytes,
            } => {
                let progress = format_download_progress(*downloaded_bytes, *total_bytes);
                if is_currently_playing {
                    format!("播放中，后台下载 {progress}，暂不可拖动: {title}")
                } else {
                    format!("预缓冲中 {progress}，达到可播后自动开始: {title}")
                }
            }
            AudioLoadStage::PreparingPlayback => {
                if hint.seekable {
                    format!("下载完成，可拖动: {title}")
                } else {
                    format!("预缓冲完成，开始边下边播: {title}")
                }
            }
            AudioLoadStage::Retrying {
                attempt,
                max_attempts,
            } => {
                if is_currently_playing {
                    format!("播放中断流，正在重试({attempt}/{max_attempts}): {title}")
                } else {
                    format!("预缓冲失败，正在重试({attempt}/{max_attempts}): {title}")
                }
            }
        };
    }

    match stage {
        AudioLoadStage::CacheHit => format!("缓存命中，准备播放: {title}"),
        AudioLoadStage::DownloadQueued => format!("缓存未命中，开始下载: {title}"),
        AudioLoadStage::Downloading {
            downloaded_bytes,
            total_bytes,
        } => format!(
            "下载中 {}: {title}",
            format_download_progress(*downloaded_bytes, *total_bytes)
        ),
        AudioLoadStage::PreparingPlayback => format!("下载完成，准备播放: {title}"),
        AudioLoadStage::Retrying {
            attempt,
            max_attempts,
        } => {
            format!("下载失败，正在重试({attempt}/{max_attempts}): {title}")
        }
    }
}

pub struct AudioEventCtx<'a> {
    pub request_tracker: &'a mut RequestTracker<RequestKey>,
    pub song_request_titles: &'a mut std::collections::HashMap<i64, String>,
    pub req_id: &'a mut u64,
    pub next_song_cache: &'a mut NextSongCacheManager,
}

fn restore_pending_seek_if_possible(
    app: &mut App,
    effects: &mut CoreEffects,
    stream_hint: &AudioStreamHint,
) -> Option<u64> {
    let seek_ms = app.pending_seek_ms?;
    if !stream_hint.seekable {
        return None;
    }

    let seek_ms = app.pending_seek_ms.take().unwrap_or(seek_ms);
    effects.send_audio_warn(
        AudioCommand::SeekToMs(seek_ms),
        "AudioWorker 通道已关闭：SeekToMs 发送失败",
    );

    let now = Instant::now();
    app.play_started_at = Some(now - Duration::from_millis(seek_ms));
    app.play_paused_at = if app.paused { Some(now) } else { None };
    app.play_paused_accum_ms = 0;
    Some(seek_ms)
}

/// 处理音频事件
pub async fn handle_audio_event(
    app: &mut App,
    evt: AudioEvent,
    ctx: &mut AudioEventCtx<'_>,
    effects: &mut CoreEffects,
) {
    match evt {
        AudioEvent::Loading {
            song_id,
            title,
            stage,
            stream_hint,
        } => {
            let is_currently_playing = app.play_id.is_some() && app.play_song_id == Some(song_id);
            app.play_song_id = Some(song_id);
            app.play_stream_hint = stream_hint.clone();
            app.play_status = format_loading_status(
                &title,
                &stage,
                app.play_stream_hint.as_ref(),
                is_currently_playing,
            );
        }
        AudioEvent::NowPlaying {
            song_id,
            play_id,
            title,
            duration_ms,
            stream_hint,
        } => {
            // 保存待恢复的播放位置（在重置之前）
            let seek_to = app.pending_seek_ms;

            // 记录旧的播放进度
            let old_elapsed_ms = if let Some(started) = app.play_started_at {
                let elapsed = started.elapsed().as_millis() as u64;
                if app.paused {
                    elapsed.saturating_sub(app.play_paused_accum_ms)
                } else {
                    elapsed
                }
            } else {
                0
            };

            tracing::info!(
                song_id,
                play_id,
                title = %title,
                old_elapsed_ms = old_elapsed_ms / 1000,
                paused = app.paused,
                paused_accum_ms = app.play_paused_accum_ms,
                seek_to = ?seek_to,
                "🎵 [PlayerAudio] NowPlaying START"
            );

            app.now_playing = Some(title);
            app.paused = false;
            app.play_status = format_playback_status(false, Some(&stream_hint));
            app.play_started_at = Some(Instant::now());
            app.play_total_ms = duration_ms;
            app.play_stream_hint = Some(stream_hint.clone());
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = Some(play_id);
            app.play_song_id = Some(song_id);
            app.play_error_count = 0;
            effects.send_audio_warn(
                AudioCommand::SetVolume(app.volume),
                "AudioWorker 通道已关闭：SetVolume 发送失败",
            );

            // 记录是否恢复了播放进度
            let restored_seek_ms = restore_pending_seek_if_possible(app, effects, &stream_hint);
            if let Some(seek_ms) = restored_seek_ms {
                tracing::info!("🎵 [PlayerAudio] 恢复播放进度: {}ms", seek_ms);
            }

            tracing::warn!(
                "🎵 [PlayerAudio] NowPlaying END: play_started_at 已重置为当前时间，播放进度已从 {}s {}",
                old_elapsed_ms / 1000,
                if let Some(seek_ms) = restored_seek_ms {
                    format!("恢复到 {}s", seek_ms / 1000)
                } else if let Some(seek_ms) = seek_to {
                    format!("待缓存完成后恢复到 {}s", seek_ms / 1000)
                } else {
                    "重置为 0s".to_string()
                }
            );

            app.lyrics_song_id = None;
            app.lyrics.clear();
            app.lyrics_status = "加载歌词...".to_owned();
            let id = ctx
                .request_tracker
                .issue(RequestKey::Lyric, || utils::next_id(ctx.req_id));
            effects.send_netease_hi_warn(
                NeteaseCommand::Lyric {
                    req_id: id,
                    song_id,
                },
                "NeteaseActor 通道已关闭：Lyric 发送失败",
            );
        }
        AudioEvent::PlaybackHint {
            song_id,
            play_id,
            hint,
        } => {
            if app.play_id != Some(play_id) || app.play_song_id != Some(song_id) {
                return;
            }

            let became_seekable = app
                .play_stream_hint
                .as_ref()
                .is_some_and(|prev| !prev.seekable && hint.seekable);
            app.play_stream_hint = Some(hint.clone());
            app.play_status = if became_seekable {
                if app.paused {
                    "已暂停（已缓存完成，可拖动）".to_owned()
                } else {
                    "播放中（已缓存完成，可拖动）".to_owned()
                }
            } else {
                format_playback_status(app.paused, Some(&hint))
            };
            let _ = restore_pending_seek_if_possible(app, effects, &hint);
        }
        AudioEvent::Paused(p) => {
            tracing::info!(
                paused = p,
                old_paused = app.paused,
                "🎵 [PlayerAudio] 收到 Paused 事件"
            );

            app.paused = p;
            app.play_status = format_playback_status(p, app.play_stream_hint.as_ref());

            tracing::debug!(
                play_status = %app.play_status,
                "🎵 [PlayerAudio] 更新播放状态"
            );

            if p {
                app.play_paused_at = Some(std::time::Instant::now());
            } else if let Some(t) = app.play_paused_at.take() {
                app.play_paused_accum_ms = app
                    .play_paused_accum_ms
                    .saturating_add(t.elapsed().as_millis() as u64);
            }
        }
        AudioEvent::Stopped => {
            app.paused = false;
            app.play_status = "已停止".to_owned();
            app.play_started_at = None;
            app.play_total_ms = None;
            app.play_stream_hint = None;
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = None;
            app.play_song_id = None;
            app.play_error_count = 0;
        }
        AudioEvent::CacheCleared { files, bytes } => {
            app.settings_status = format!(
                "已清除音频缓存：{} 个文件，释放 {} MB",
                files,
                bytes / 1024 / 1024
            );
            tracing::info!(files, bytes, "音频缓存已清除");
        }
        AudioEvent::Ended { play_id } => {
            if app.play_id != Some(play_id) {
                return;
            }
            play_next(
                app,
                ctx.request_tracker,
                ctx.song_request_titles,
                ctx.req_id,
                ctx.next_song_cache,
                effects,
            )
            .await;
        }
        AudioEvent::NeedsReload => {
            // 计算当前播放进度
            let current_elapsed_ms = if let Some(started) = app.play_started_at {
                let elapsed = started.elapsed().as_millis() as u64;
                if app.paused {
                    elapsed.saturating_sub(app.play_paused_accum_ms)
                } else {
                    elapsed
                }
            } else {
                0
            };

            tracing::info!(
                play_song_id = ?app.play_song_id,
                elapsed_ms = current_elapsed_ms / 1000,
                paused = app.paused,
                paused_accum_ms = app.play_paused_accum_ms,
                play_total_ms = ?app.play_total_ms,
                "🎵 [PlayerAudio] 收到 NeedsReload 事件，重新加载音频"
            );

            // 保存播放进度，用于重新加载后恢复
            if current_elapsed_ms > 0 {
                app.pending_seek_ms = Some(current_elapsed_ms);
                tracing::info!("🎵 [PlayerAudio] 保存播放进度: {}ms", current_elapsed_ms);
            }

            // 检查是否有有效的歌曲可以播放
            let song_id = match app
                .play_song_id
                .or_else(|| app.play_queue.current().map(|s| s.id))
            {
                Some(id) => id,
                None => {
                    tracing::warn!("🎵 [PlayerAudio] 没有可播放的歌曲");
                    app.play_status = "无歌曲可播放".to_string();
                    return;
                }
            };

            // 获取歌曲标题用于请求
            let current_song = app.play_queue.current();
            let title = current_song
                .map(|s| format!("{} - {}", s.name, s.artists))
                .or_else(|| app.now_playing.clone())
                .unwrap_or_else(|| "未知歌曲".to_string());

            tracing::info!(
                song_id,
                title = %title,
                "🎵 [PlayerAudio] 重新请求播放链接"
            );

            app.play_status = format!("加载中: {}", title);
            app.play_stream_hint = None;

            // 清理旧的请求记录并重新请求
            ctx.song_request_titles.clear();
            let req_id = ctx
                .request_tracker
                .issue(RequestKey::SongUrl, || utils::next_id(ctx.req_id));
            ctx.song_request_titles.insert(song_id, title.clone());

            effects.send_netease_hi_warn(
                NeteaseCommand::SongUrl {
                    req_id,
                    id: song_id,
                    br: app.play_br,
                },
                "NeteaseActor 通道已关闭：SongUrl 发送失败",
            );
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");

            let retryable = e.is_retryable();
            if retryable {
                app.play_error_count = app.play_error_count.saturating_add(1);
                let current_song = app.play_queue.current();
                if app.play_error_count <= 2
                    && let Some(song_id) = app.play_song_id.or_else(|| current_song.map(|s| s.id))
                {
                    let title = current_song
                        .map(|s| format!("{} - {}", s.name, s.artists))
                        .or_else(|| app.now_playing.clone())
                        .unwrap_or_else(|| "未知歌曲".to_owned());
                    app.play_status = format!("播放失败，正在重试({}/2)...", app.play_error_count);
                    ctx.song_request_titles.clear();
                    let id = ctx
                        .request_tracker
                        .issue(RequestKey::SongUrl, || utils::next_id(ctx.req_id));
                    ctx.song_request_titles.insert(song_id, title);
                    effects.send_netease_hi(crate::netease::actor::NeteaseCommand::SongUrl {
                        req_id: id,
                        id: song_id,
                        br: app.play_br,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{format_loading_status, handle_audio_event};
    use crate::audio_worker::{AudioBufferState, AudioEvent, AudioLoadStage, AudioStreamHint};
    use crate::core::CoreEffects;
    use crate::core::infra::{NextSongCacheManager, RequestKey, RequestTracker};
    use crate::features::player::audio::AudioEventCtx;

    #[test]
    fn loading_status_formats_percent_with_total_bytes() {
        let status = format_loading_status(
            "Test Song",
            &AudioLoadStage::Downloading {
                downloaded_bytes: 512 * 1024,
                total_bytes: Some(1024 * 1024),
            },
            None,
            false,
        );

        assert!(status.contains("下载中 50%"));
        assert!(status.contains("512.0 KB/1.0 MB"));
    }

    #[test]
    fn loading_status_formats_without_total_bytes() {
        let status = format_loading_status(
            "Test Song",
            &AudioLoadStage::Downloading {
                downloaded_bytes: 3 * 1024 * 1024,
                total_bytes: None,
            },
            None,
            false,
        );

        assert_eq!(status, "下载中 3.0 MB: Test Song");
    }

    #[tokio::test]
    async fn loading_event_updates_play_status() {
        let mut app = crate::app::App::default();
        let mut request_tracker = RequestTracker::<RequestKey>::new();
        let mut song_request_titles = std::collections::HashMap::new();
        let mut req_id = 1u64;
        let mut next_song_cache = NextSongCacheManager::default();
        let mut effects = CoreEffects::default();
        let mut ctx = AudioEventCtx {
            request_tracker: &mut request_tracker,
            song_request_titles: &mut song_request_titles,
            req_id: &mut req_id,
            next_song_cache: &mut next_song_cache,
        };

        handle_audio_event(
            &mut app,
            AudioEvent::Loading {
                song_id: 7,
                title: "Test Song".to_owned(),
                stage: AudioLoadStage::PreparingPlayback,
                stream_hint: None,
            },
            &mut ctx,
            &mut effects,
        )
        .await;

        assert_eq!(app.play_song_id, Some(7));
        assert_eq!(app.play_status, "下载完成，准备播放: Test Song");
    }

    #[tokio::test]
    async fn streaming_now_playing_updates_seekability_status() {
        let mut app = crate::app::App::default();
        let mut request_tracker = RequestTracker::<RequestKey>::new();
        let mut song_request_titles = std::collections::HashMap::new();
        let mut req_id = 1u64;
        let mut next_song_cache = NextSongCacheManager::default();
        let mut effects = CoreEffects::default();
        let mut ctx = AudioEventCtx {
            request_tracker: &mut request_tracker,
            song_request_titles: &mut song_request_titles,
            req_id: &mut req_id,
            next_song_cache: &mut next_song_cache,
        };

        handle_audio_event(
            &mut app,
            AudioEvent::NowPlaying {
                song_id: 7,
                play_id: 9,
                title: "Test Song".to_owned(),
                duration_ms: Some(240_000),
                stream_hint: AudioStreamHint::progressive(
                    AudioBufferState::Ready,
                    false,
                    256 * 1024,
                    Some(1024 * 1024),
                ),
            },
            &mut ctx,
            &mut effects,
        )
        .await;

        assert_eq!(app.play_id, Some(9));
        assert_eq!(app.play_status, "播放中（边下边播，暂不可拖动）");
        assert!(!app.can_seek());
    }
}
