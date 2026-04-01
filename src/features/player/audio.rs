use crate::core::prelude::{
    app::App,
    audio::{AudioCommand, AudioEvent, AudioLoadStage},
    effects::CoreEffects,
    infra::{NextSongCacheManager, RequestKey, RequestTracker},
    netease::NeteaseCommand,
};
use crate::core::utils;
use crate::features::player::playback::play_next;

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

fn format_loading_status(title: &str, stage: &AudioLoadStage) -> String {
    match stage {
        AudioLoadStage::CacheHit => format!("缓存命中，准备播放: {title}"),
        AudioLoadStage::DownloadQueued => format!("缓存未命中，开始下载: {title}"),
        AudioLoadStage::Downloading {
            downloaded_bytes,
            total_bytes,
        } => match total_bytes.filter(|total| *total > 0) {
            Some(total_bytes) => {
                let percent = downloaded_bytes.saturating_mul(100) / total_bytes;
                format!(
                    "下载中 {percent}% ({}/{}): {title}",
                    format_bytes(*downloaded_bytes),
                    format_bytes(total_bytes)
                )
            }
            None => format!("下载中 {}: {title}", format_bytes(*downloaded_bytes)),
        },
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
        } => {
            app.play_song_id = Some(song_id);
            app.play_status = format_loading_status(&title, &stage);
        }
        AudioEvent::NowPlaying {
            song_id,
            play_id,
            title,
            duration_ms,
        } => {
            // 保存待恢复的播放位置（在重置之前）
            let seek_to = app.pending_seek_ms.take();

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
            app.play_status = "播放中".to_owned();
            app.play_started_at = Some(std::time::Instant::now());
            app.play_total_ms = duration_ms;
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
            let restored_seek_ms = seek_to;

            // 如果有待恢复的播放位置，发送 seek 命令
            if let Some(seek_ms) = restored_seek_ms {
                tracing::info!("🎵 [PlayerAudio] 恢复播放进度: {}ms", seek_ms);
                effects.send_audio_warn(
                    AudioCommand::SeekToMs(seek_ms),
                    "AudioWorker 通道已关闭：SeekToMs 发送失败",
                );
                // 更新 play_started_at 以匹配 seek 位置
                app.play_started_at =
                    Some(std::time::Instant::now() - std::time::Duration::from_millis(seek_ms));
            }

            tracing::warn!(
                "🎵 [PlayerAudio] NowPlaying END: play_started_at 已重置为当前时间，播放进度已从 {}s {}",
                old_elapsed_ms / 1000,
                if let Some(seek_ms) = restored_seek_ms {
                    format!("恢复到 {}s", seek_ms / 1000)
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
        AudioEvent::Paused(p) => {
            tracing::info!(
                paused = p,
                old_paused = app.paused,
                "🎵 [PlayerAudio] 收到 Paused 事件"
            );

            app.paused = p;
            app.play_status = (if p { "已暂停" } else { "播放中" }).to_owned();

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
    use crate::audio_worker::{AudioEvent, AudioLoadStage};
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
            },
            &mut ctx,
            &mut effects,
        )
        .await;

        assert_eq!(app.play_song_id, Some(7));
        assert_eq!(app.play_status, "下载完成，准备播放: Test Song");
    }
}
