use reqwest::blocking::Client;
use rodio::OutputStreamBuilder;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::messages::{AudioCommand, AudioEvent};
use super::player::{play_track, seek_to_ms, PlayerState};
use super::cache::AudioCache;

pub fn spawn_audio_worker(
    data_dir: PathBuf,
) -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<AudioCommand>();
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>();

    thread::spawn(move || {
        let http = match Client::builder().timeout(Duration::from_secs(30)).build() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(err = %e, "初始化 HTTP 客户端失败");
                let _ = tx_evt.send(AudioEvent::Error(format!("初始化 HTTP 客户端失败: {e}")));
                return;
            }
        };
        let stream = match OutputStreamBuilder::open_default_stream() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(err = %e, "初始化音频输出失败");
                let _ = tx_evt.send(AudioEvent::Error(format!("初始化音频输出失败: {e}")));
                return;
            }
        };
        let mixer = stream.mixer().clone();
        let cache = AudioCache::new(&data_dir);
        tracing::info!(data_dir = %data_dir.display(), "AudioWorker 已启动");
        let mut state = PlayerState::new(mixer, stream, cache);

        while let Ok(cmd) = rx_cmd.recv() {
            match cmd {
                AudioCommand::PlayTrack { id, br, url, title } => {
                    tracing::info!(song_id = id, br, title = %title, "开始播放请求");
                    match play_track(&http, &tx_evt, &mut state, id, br, &url, &title) {
                        Ok((play_id, duration_ms)) => {
                            let _ = tx_evt.send(AudioEvent::NowPlaying {
                                song_id: id,
                                play_id,
                                title,
                                duration_ms,
                            });
                        }
                        Err(e) => {
                            tracing::error!(song_id = id, br, title = %title, err = %e, "播放失败");
                            let _ = tx_evt.send(AudioEvent::Error(e));
                        }
                    }
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
                    let (files, bytes) = state.cache.clear_all(state.path.as_deref());
                    tracing::info!(files, bytes, "音频缓存清理完成");
                    let _ = tx_evt.send(AudioEvent::CacheCleared { files, bytes });
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}
