use crate::core::prelude::{
    app::App,
    audio::{AudioCommand, AudioEvent},
    effects::CoreEffects,
    infra::NextSongCacheManager,
    netease::NeteaseCommand,
};
use crate::core::utils;
use crate::features::player::playback::play_next;

pub struct AudioEventCtx<'a> {
    pub pending_song_url: &'a mut Option<(u64, String)>,
    pub pending_lyric: &'a mut Option<(u64, i64)>,
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
        AudioEvent::NowPlaying {
            song_id,
            play_id,
            title,
            duration_ms,
        } => {
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

            app.lyrics_song_id = None;
            app.lyrics.clear();
            app.lyrics_status = "加载歌词...".to_owned();
            let id = utils::next_id(ctx.req_id);
            *ctx.pending_lyric = Some((id, song_id));
            effects.send_netease_hi_warn(
                NeteaseCommand::Lyric {
                    req_id: id,
                    song_id,
                },
                "NeteaseActor 通道已关闭：Lyric 发送失败",
            );
        }
        AudioEvent::Paused(p) => {
            app.paused = p;
            app.play_status = (if p { "已暂停" } else { "播放中" }).to_owned();
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
                ctx.pending_song_url,
                ctx.req_id,
                ctx.next_song_cache,
                effects,
            )
            .await;
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");

            let retryable = e.contains("下载音频失败");
            if retryable {
                app.play_error_count = app.play_error_count.saturating_add(1);
                if app.play_error_count <= 2
                    && let Some(song_id) = app.play_song_id.or_else(|| {
                        app.queue_pos
                            .and_then(|pos| app.queue.get(pos))
                            .map(|s| s.id)
                    })
                {
                    let title = app
                        .queue_pos
                        .and_then(|pos| app.queue.get(pos))
                        .map(|s| format!("{} - {}", s.name, s.artists))
                        .or_else(|| app.now_playing.clone())
                        .unwrap_or_else(|| "未知歌曲".to_owned());
                    app.play_status = format!("播放失败，正在重试({}/2)...", app.play_error_count);
                    let id = utils::next_id(ctx.req_id);
                    *ctx.pending_song_url = Some((id, title));
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
