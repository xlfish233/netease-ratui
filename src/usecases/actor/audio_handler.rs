use crate::app::App;
use crate::audio_worker::AudioCommand;
use crate::netease::actor::NeteaseCommand;
use crate::usecases::actor::playback;

use super::utils;
use tokio::sync::mpsc;

/// 处理音频事件
pub(super) async fn handle_audio_event(
    app: &mut App,
    evt: crate::audio_worker::AudioEvent,
    tx_netease: &mpsc::Sender<NeteaseCommand>,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    pending_song_url: &mut Option<(u64, String)>,
    pending_lyric: &mut Option<(u64, i64)>,
    req_id: &mut u64,
    next_song_cache: &mut super::next_song_cache::NextSongCacheManager,
    tx_netease_lo: &mpsc::Sender<NeteaseCommand>,
) {
    match evt {
        crate::audio_worker::AudioEvent::NowPlaying {
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
            if tx_audio.send(AudioCommand::SetVolume(app.volume)).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：SetVolume 发送失败");
            }

            app.lyrics_song_id = None;
            app.lyrics.clear();
            app.lyrics_status = "加载歌词...".to_owned();
            let id = utils::next_id(req_id);
            *pending_lyric = Some((id, song_id));
            if let Err(e) = tx_netease
                .send(NeteaseCommand::Lyric {
                    req_id: id,
                    song_id,
                })
                .await
            {
                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：Lyric 发送失败");
            }
        }
        crate::audio_worker::AudioEvent::Paused(p) => {
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
        crate::audio_worker::AudioEvent::Stopped => {
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
        crate::audio_worker::AudioEvent::CacheCleared { files, bytes } => {
            app.settings_status = format!(
                "已清除音频缓存：{} 个文件，释放 {} MB",
                files,
                bytes / 1024 / 1024
            );
            tracing::info!(files, bytes, "音频缓存已清除");
        }
        crate::audio_worker::AudioEvent::Ended { play_id } => {
            if app.play_id != Some(play_id) {
                return;
            }
            playback::play_next(
                app,
                tx_netease,
                pending_song_url,
                req_id,
                next_song_cache,
                tx_netease_lo,
            )
            .await;
        }
        crate::audio_worker::AudioEvent::Error(e) => {
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
                    let id = utils::next_id(req_id);
                    *pending_song_url = Some((id, title));
                    let _ = tx_netease
                        .send(NeteaseCommand::SongUrl {
                            req_id: id,
                            id: song_id,
                            br: app.play_br,
                        })
                        .await;
                }
            }
        }
    }
}
