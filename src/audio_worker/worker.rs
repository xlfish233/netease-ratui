use rodio::OutputStreamBuilder;
use std::path::PathBuf;
use tokio::select;
use tokio::sync::mpsc;

use super::messages::{AudioCommand, AudioEvent};
use super::player::{PlayerState, play_path, seek_to_ms};
use super::transfer::{
    CacheKey, Priority, TransferCommand, TransferConfig, TransferEvent,
    spawn_transfer_actor_with_config,
};

struct PendingPlay {
    token: u64,
    key: CacheKey,
    title: String,
    url: String,
    retries: u8,
}

pub fn spawn_audio_worker_with_config(
    data_dir: PathBuf,
    config: TransferConfig,
) -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<AudioCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>(64);

    // Spawn TransferActor on tokio runtime if available; otherwise it will self-host.
    let (tx_transfer, mut rx_transfer) = spawn_transfer_actor_with_config(data_dir.clone(), config);

    let run = async move {
        let stream = match OutputStreamBuilder::open_default_stream() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(err = %e, "初始化音频输出失败");
                let _ = tx_evt
                    .send(AudioEvent::Error(format!("初始化音频输出失败: {e}")))
                    .await;
                return;
            }
        };
        let mixer = stream.mixer().clone();
        let mut state = PlayerState::new(mixer, stream);

        tracing::info!(data_dir = %data_dir.display(), "AudioWorker 已启动");

        let mut next_token: u64 = 1;
        let mut pending_play: Option<PendingPlay> = None;
        let mut transfer_closed = false;

        loop {
            select! {
                biased;
                maybe_evt = rx_transfer.recv(), if !transfer_closed => {
                    match maybe_evt {
                        Some(evt) => {
                            match evt {
                                TransferEvent::Ready { token, key, path } => {
                                    let Some(mut p) = pending_play.take().filter(|p| p.token == token) else {
                                        continue;
                                    };

                                    match play_path(&tx_evt, &mut state, &p.title, &path) {
                                        Ok((play_id, duration_ms)) => {
                                            let _ = tx_evt.send(AudioEvent::NowPlaying {
                                                song_id: key.song_id,
                                                play_id,
                                                title: p.title.clone(),
                                                duration_ms,
                                            }).await;
                                        }
                                        Err(e) => {
                                            // If the cached file is corrupted, invalidate and retry once.
                                            if p.retries < 1 {
                                                p.retries += 1;
                                                state.stop();
                                                let _ = tx_transfer.send(TransferCommand::Invalidate { key }).await;
                                                let _ = tx_transfer.send(TransferCommand::EnsureCached {
                                                    token: p.token,
                                                    key: p.key,
                                                    url: p.url.clone(),
                                                    title: p.title.clone(),
                                                    priority: Priority::High,
                                                }).await;
                                                pending_play = Some(p);
                                                continue;
                                            }
                                            let _ = tx_evt.send(AudioEvent::Error(e)).await;
                                        }
                                    }
                                }
                                TransferEvent::Error { token, message } => {
                                    if pending_play.as_ref().is_some_and(|p| p.token == token) {
                                        pending_play = None;
                                        let _ = tx_evt.send(AudioEvent::Error(message)).await;
                                    }
                                }
                                TransferEvent::CacheCleared { files, bytes } => {
                                    let _ = tx_evt.send(AudioEvent::CacheCleared { files, bytes }).await;
                                }
                            }
                        }
                        None => {
                            transfer_closed = true;
                        }
                    }
                }
                maybe_cmd = rx_cmd.recv() => {
                    let Some(cmd) = maybe_cmd else {
                        break;
                    };

                    match cmd {
                        AudioCommand::PlayTrack { id, br, url, title } => {
                            tracing::info!(song_id = id, br, title = %title, "开始播放请求");
                            state.stop();

                            let token = next_token;
                            next_token = next_token.wrapping_add(1).max(1);

                            let key = CacheKey { song_id: id, br };
                            pending_play = Some(PendingPlay {
                                token,
                                key,
                                title: title.clone(),
                                url: url.clone(),
                                retries: 0,
                            });

                            let _ = tx_transfer.send(TransferCommand::EnsureCached {
                                token,
                                key,
                                url,
                                title,
                                priority: Priority::High,
                            }).await;
                        }
                        AudioCommand::TogglePause => {
                            if let Some(sink) = state.sink.as_ref() {
                                if sink.is_paused() {
                                    sink.play();
                                    state.paused = false;
                                    let _ = tx_evt.send(AudioEvent::Paused(false)).await;
                                } else {
                                    sink.pause();
                                    state.paused = true;
                                    let _ = tx_evt.send(AudioEvent::Paused(true)).await;
                                }
                            }
                        }
                        AudioCommand::Stop => {
                            pending_play = None;
                            state.stop();
                            let _ = tx_evt.send(AudioEvent::Stopped).await;
                        }
                        AudioCommand::SeekToMs(ms) => {
                            if let Err(e) = seek_to_ms(&tx_evt, &mut state, ms) {
                                tracing::warn!(ms, err = %e, "Seek 失败");
                                let _ = tx_evt.send(AudioEvent::Error(e)).await;
                            }
                        }
                        AudioCommand::SetVolume(v) => {
                            state.volume = v.clamp(0.0, 2.0);
                            if let Some(sink) = state.sink.as_ref() {
                                sink.set_volume(state.volume);
                            }
                        }
                        AudioCommand::ClearCache => {
                            tracing::info!("用户触发：清除音频缓存");
                            let _ = tx_transfer.send(TransferCommand::ClearAll {
                                keep: state.path.clone(),
                            }).await;
                        }
                        AudioCommand::SetCacheBr(br) => {
                            let _ = tx_transfer.send(TransferCommand::PurgeNotBr {
                                br,
                                keep: state.path.clone(),
                            }).await;
                        }
                        AudioCommand::PrefetchAudio { id, br, url, title } => {
                            tracing::info!(song_id = id, br, title = %title, "开始预缓存");
                            let key = CacheKey { song_id: id, br };
                            let _ = tx_transfer.send(TransferCommand::EnsureCached {
                                token: 0,
                                key,
                                url,
                                title,
                                priority: Priority::Low,
                            }).await;
                        }
                    }
                }
            }
        }
    };

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(run);
    });

    (tx_cmd, rx_evt)
}
