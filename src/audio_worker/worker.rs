use rodio::OutputStreamBuilder;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::messages::{AudioCommand, AudioEvent};
use super::player::{PlayerState, play_path, seek_to_ms};
use super::transfer::{CacheKey, Priority, TransferCommand, TransferEvent, spawn_transfer_actor};

struct PendingPlay {
    token: u64,
    key: CacheKey,
    title: String,
    url: String,
    retries: u8,
}

pub fn spawn_audio_worker(
    data_dir: PathBuf,
) -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<AudioCommand>();
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>();

    // Spawn TransferActor on tokio runtime if available; otherwise it will self-host.
    let (tx_transfer, rx_transfer) = spawn_transfer_actor(data_dir.clone());

    thread::spawn(move || {
        let stream = match OutputStreamBuilder::open_default_stream() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(err = %e, "初始化音频输出失败");
                let _ = tx_evt.send(AudioEvent::Error(format!("初始化音频输出失败: {e}")));
                return;
            }
        };
        let mixer = stream.mixer().clone();
        let mut state = PlayerState::new(mixer, stream);

        tracing::info!(data_dir = %data_dir.display(), "AudioWorker 已启动");

        let mut next_token: u64 = 1;
        let mut pending_play: Option<PendingPlay> = None;
        let mut rx_transfer = rx_transfer;

        loop {
            // Drain transfer events first to reduce playback latency.
            loop {
                match rx_transfer.try_recv() {
                    Ok(evt) => match evt {
                        TransferEvent::Ready { token, key, path } => {
                            let Some(mut p) = pending_play.take().filter(|p| p.token == token)
                            else {
                                continue;
                            };

                            match play_path(&tx_evt, &mut state, &p.title, &path) {
                                Ok((play_id, duration_ms)) => {
                                    let _ = tx_evt.send(AudioEvent::NowPlaying {
                                        song_id: key.song_id,
                                        play_id,
                                        title: p.title.clone(),
                                        duration_ms,
                                    });
                                }
                                Err(e) => {
                                    // If the cached file is corrupted, invalidate and retry once.
                                    if p.retries < 1 {
                                        p.retries += 1;
                                        state.stop();
                                        let _ = tx_transfer
                                            .blocking_send(TransferCommand::Invalidate { key });
                                        let _ = tx_transfer.blocking_send(
                                            TransferCommand::EnsureCached {
                                                token: p.token,
                                                key: p.key,
                                                url: p.url.clone(),
                                                title: p.title.clone(),
                                                priority: Priority::High,
                                            },
                                        );
                                        pending_play = Some(p);
                                        continue;
                                    }
                                    let _ = tx_evt.send(AudioEvent::Error(e));
                                }
                            }
                        }
                        TransferEvent::Error { token, message } => {
                            if pending_play.as_ref().is_some_and(|p| p.token == token) {
                                pending_play = None;
                                let _ = tx_evt.send(AudioEvent::Error(message));
                            }
                        }
                        TransferEvent::CacheCleared { files, bytes } => {
                            let _ = tx_evt.send(AudioEvent::CacheCleared { files, bytes });
                        }
                    },
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                }
            }

            let cmd = match rx_cmd.recv_timeout(Duration::from_millis(50)) {
                Ok(cmd) => cmd,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
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

                    let _ = tx_transfer.blocking_send(TransferCommand::EnsureCached {
                        token,
                        key,
                        url,
                        title,
                        priority: Priority::High,
                    });
                }
                AudioCommand::TogglePause => {
                    if let Some(sink) = state.sink.as_ref() {
                        if sink.is_paused() {
                            sink.play();
                            state.paused = false;
                            let _ = tx_evt.send(AudioEvent::Paused(false));
                        } else {
                            sink.pause();
                            state.paused = true;
                            let _ = tx_evt.send(AudioEvent::Paused(true));
                        }
                    }
                }
                AudioCommand::Stop => {
                    pending_play = None;
                    state.stop();
                    let _ = tx_evt.send(AudioEvent::Stopped);
                }
                AudioCommand::SeekToMs(ms) => {
                    if let Err(e) = seek_to_ms(&tx_evt, &mut state, ms) {
                        tracing::warn!(ms, err = %e, "Seek 失败");
                        let _ = tx_evt.send(AudioEvent::Error(e));
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
                    let _ = tx_transfer.blocking_send(TransferCommand::ClearAll {
                        keep: state.path.clone(),
                    });
                }
                AudioCommand::SetCacheBr(br) => {
                    let _ = tx_transfer.blocking_send(TransferCommand::PurgeNotBr {
                        br,
                        keep: state.path.clone(),
                    });
                }
                AudioCommand::PrefetchAudio { id, br, url, title } => {
                    tracing::info!(song_id = id, br, title = %title, "开始预缓存");
                    let key = CacheKey { song_id: id, br };
                    let _ = tx_transfer.blocking_send(TransferCommand::EnsureCached {
                        token: 0,
                        key,
                        url,
                        title,
                        priority: Priority::Low,
                    });
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}
