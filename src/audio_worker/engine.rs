use crate::error::MessageError;
use rodio::OutputStreamBuilder;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;

use super::AudioSettings;
use super::fade::Crossfade;
use super::messages::{AudioCommand, AudioEvent, AudioLoadStage};
use super::player::{PlayerState, seek_to_ms};
use super::transfer::{
    CacheKey, Priority, TransferCommand, TransferConfig, TransferEvent, TransferReceiver,
    TransferSender, spawn_transfer_actor_with_config,
};

struct PendingPlay {
    token: u64,
    key: CacheKey,
    title: String,
    url: String,
    retries: u8,
}

struct AudioEngine {
    tx_evt: mpsc::Sender<AudioEvent>,
    rx_cmd: mpsc::Receiver<AudioCommand>,
    tx_transfer: TransferSender,
    rx_transfer: TransferReceiver,
    state: PlayerState,
    pending_play: Option<PendingPlay>,
    next_token: u64,
    transfer_closed: bool,
    crossfade_ms: u64,
    fade: Option<Crossfade>,
    ended_reported_play_id: Option<u64>,
}

impl AudioEngine {
    fn new(
        tx_evt: mpsc::Sender<AudioEvent>,
        rx_cmd: mpsc::Receiver<AudioCommand>,
        tx_transfer: TransferSender,
        rx_transfer: TransferReceiver,
        state: PlayerState,
        settings: AudioSettings,
    ) -> Self {
        Self {
            tx_evt,
            rx_cmd,
            tx_transfer,
            rx_transfer,
            state,
            pending_play: None,
            next_token: 1,
            transfer_closed: false,
            crossfade_ms: settings.crossfade_ms,
            fade: None,
            ended_reported_play_id: None,
        }
    }

    async fn run(mut self) {
        let mut fade_tick = tokio::time::interval(Duration::from_millis(20));
        let mut end_tick = tokio::time::interval(Duration::from_millis(200));

        loop {
            select! {
                biased;
                _ = fade_tick.tick(), if self.fade.is_some() => {
                    self.tick_fade();
                }
                _ = end_tick.tick() => {
                    self.tick_end().await;
                }
                maybe_evt = self.rx_transfer.recv(), if !self.transfer_closed => {
                    match maybe_evt {
                        Some(evt) => self.handle_transfer_event(evt).await,
                        None => {
                            self.transfer_closed = true;
                        }
                    }
                }
                maybe_cmd = self.rx_cmd.recv() => {
                    let Some(cmd) = maybe_cmd else {
                        break;
                    };
                    self.handle_audio_command(cmd).await;
                }
            }
        }
    }

    fn tick_fade(&mut self) {
        if let Some(fade) = &mut self.fade
            && fade.apply(self.state.volume())
        {
            self.fade = None;
            if let Some(sink) = self.state.current_sink() {
                sink.set_volume(self.state.volume());
            }
        }
    }

    fn clear_fade(&mut self) {
        if let Some(fade) = self.fade.take() {
            fade.stop();
        }
    }

    async fn tick_end(&mut self) {
        // Poll for natural end of the *current* sink; avoids spawning an OS thread per track.
        let Some(sink) = self.state.current_sink() else {
            return;
        };
        let play_id = self.state.play_id();
        if self.ended_reported_play_id == Some(play_id) {
            return;
        }

        // If the user paused, don't emit "ended" while paused (seeks/pauses can transiently stop output).
        if self.state.paused() {
            return;
        }

        if sink.empty() {
            self.ended_reported_play_id = Some(play_id);
            tracing::debug!(play_id, "detected sink ended");
            let _ = self.tx_evt.send(AudioEvent::Ended { play_id }).await;
        }
    }

    async fn handle_transfer_event(&mut self, evt: TransferEvent) {
        match evt {
            TransferEvent::CacheHit { token, key } => {
                self.emit_loading_event(token, key.song_id, AudioLoadStage::CacheHit)
                    .await;
            }
            TransferEvent::DownloadQueued { token, key } => {
                self.emit_loading_event(token, key.song_id, AudioLoadStage::DownloadQueued)
                    .await;
            }
            TransferEvent::Progress {
                token,
                key,
                downloaded_bytes,
                total_bytes,
            } => {
                self.emit_loading_event(
                    token,
                    key.song_id,
                    AudioLoadStage::Downloading {
                        downloaded_bytes,
                        total_bytes,
                    },
                )
                .await;
            }
            TransferEvent::Retrying {
                token,
                key,
                attempt,
                max_attempts,
            } => {
                self.emit_loading_event(
                    token,
                    key.song_id,
                    AudioLoadStage::Retrying {
                        attempt,
                        max_attempts,
                    },
                )
                .await;
            }
            TransferEvent::Ready { token, key, path } => {
                if let Some(pending) = self.pending_play.as_ref()
                    && pending.token != token
                {
                    tracing::debug!(
                        token,
                        pending_token = pending.token,
                        song_id = key.song_id,
                        "cache ready token mismatch"
                    );
                }
                self.emit_loading_event(token, key.song_id, AudioLoadStage::PreparingPlayback)
                    .await;
                let Some(mut p) = self.pending_play.take().filter(|p| p.token == token) else {
                    return;
                };

                tracing::info!(
                    song_id = key.song_id,
                    br = key.br,
                    path = %path.display(),
                    "cache ready"
                );
                match self.start_playback(&key, &path, &p.title) {
                    Ok(duration_ms) => {
                        self.ended_reported_play_id = None;
                        let _ = self
                            .tx_evt
                            .send(AudioEvent::NowPlaying {
                                song_id: key.song_id,
                                play_id: self.state.play_id(),
                                title: p.title.clone(),
                                duration_ms,
                            })
                            .await;
                    }
                    Err(e) => {
                        if p.retries < 1 {
                            p.retries += 1;
                            self.state.stop();
                            let _ = self
                                .tx_transfer
                                .send(TransferCommand::Invalidate { key })
                                .await;
                            let _ = self
                                .tx_transfer
                                .send(TransferCommand::EnsureCached {
                                    token: p.token,
                                    key: p.key,
                                    url: p.url.clone(),
                                    title: p.title.clone(),
                                    priority: Priority::High,
                                })
                                .await;
                            self.pending_play = Some(p);
                            return;
                        }
                        let _ = self
                            .tx_evt
                            .send(AudioEvent::Error(MessageError::other(e)))
                            .await;
                    }
                }
            }
            TransferEvent::Error { token, message } => {
                tracing::warn!(token, err = %message, "cache error");
                if self.pending_play.as_ref().is_some_and(|p| p.token == token) {
                    self.pending_play = None;
                    let _ = self
                        .tx_evt
                        .send(AudioEvent::Error(MessageError::other(message)))
                        .await;
                }
            }
            TransferEvent::CacheCleared { files, bytes } => {
                let _ = self
                    .tx_evt
                    .send(AudioEvent::CacheCleared { files, bytes })
                    .await;
            }
        }
    }

    async fn emit_loading_event(&mut self, token: u64, song_id: i64, stage: AudioLoadStage) {
        let Some(pending) = self.pending_play.as_ref().filter(|p| p.token == token) else {
            return;
        };
        let _ = self
            .tx_evt
            .send(AudioEvent::Loading {
                song_id,
                title: pending.title.clone(),
                stage,
            })
            .await;
    }

    async fn handle_audio_command(&mut self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayTrack { id, br, url, title } => {
                tracing::info!(song_id = id, br, title = %title, "开始播放请求");
                if let Some(old_pending) = self.pending_play.take() {
                    tracing::debug!(
                        old_token = old_pending.token,
                        song_id = old_pending.key.song_id,
                        br = old_pending.key.br,
                        "取消旧播放请求"
                    );
                    let _ = self
                        .tx_transfer
                        .send(TransferCommand::Cancel {
                            token: old_pending.token,
                            key: old_pending.key,
                        })
                        .await;
                }
                self.clear_fade();

                let token = self.next_token;
                self.next_token = self.next_token.wrapping_add(1).max(1);

                let key = CacheKey { song_id: id, br };
                self.pending_play = Some(PendingPlay {
                    token,
                    key,
                    title: title.clone(),
                    url: url.clone(),
                    retries: 0,
                });

                tracing::info!(song_id = id, br, token, "request cache");
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::EnsureCached {
                        token,
                        key,
                        url,
                        title,
                        priority: Priority::High,
                    })
                    .await;
            }
            AudioCommand::TogglePause => {
                tracing::info!(
                    current_paused = self.state.paused(),
                    has_sink = self.state.current_sink().is_some(),
                    "🎵 [AudioEngine] 收到 TogglePause 命令"
                );

                // 新增：如果 sink 为 None，发送 NeedsReload 事件
                if self.state.current_sink().is_none() {
                    tracing::warn!("🎵 [AudioEngine] sink 为 None，需要重新加载音频");
                    let _ = self.tx_evt.send(AudioEvent::NeedsReload).await;
                    return;
                }

                let next_paused = !self.state.paused();
                self.state.set_paused(next_paused);

                tracing::debug!(
                    next_paused,
                    "🎵 [AudioEngine] 切换暂停状态: {} -> {}",
                    !next_paused,
                    next_paused
                );

                if let Some(fade) = &mut self.fade {
                    if next_paused {
                        fade.pause();
                        fade.pause_sinks();
                    } else {
                        fade.resume();
                        fade.resume_sinks();
                    }
                }
                if let Some(sink) = self.state.current_sink() {
                    if next_paused {
                        tracing::debug!("🎵 [AudioEngine] 暂停 sink");
                        sink.pause();
                    } else {
                        tracing::debug!("🎵 [AudioEngine] 恢复 sink 播放");
                        sink.play();
                    }
                } else {
                    tracing::warn!("🎵 [AudioEngine] sink 为 None，无法切换播放状态");
                }
                let _ = self.tx_evt.send(AudioEvent::Paused(next_paused)).await;
                tracing::debug!(next_paused, "🎵 [AudioEngine] 发送 Paused 事件");
            }
            AudioCommand::Stop => {
                self.pending_play = None;
                self.clear_fade();
                self.state.stop();
                self.ended_reported_play_id = None;
                let _ = self.tx_evt.send(AudioEvent::Stopped).await;
            }
            AudioCommand::SeekToMs(ms) => {
                self.clear_fade();
                tracing::trace!(
                    ms,
                    play_id = self.state.play_id(),
                    paused = self.state.paused(),
                    has_sink = self.state.current_sink().is_some(),
                    "🎵 [AudioEngine] SeekToMs"
                );
                if let Err(e) = seek_to_ms(&mut self.state, ms) {
                    tracing::warn!(ms, err = %e, "Seek 失败");
                    let _ = self
                        .tx_evt
                        .send(AudioEvent::Error(MessageError::other(e)))
                        .await;
                } else {
                    self.ended_reported_play_id = None;
                }
            }
            AudioCommand::SetVolume(v) => {
                self.state.set_volume(v.clamp(0.0, 2.0));
                if let Some(fade) = &mut self.fade {
                    let _ = fade.apply(self.state.volume());
                } else if let Some(sink) = self.state.current_sink() {
                    sink.set_volume(self.state.volume());
                }
            }
            AudioCommand::SetCrossfadeMs(ms) => {
                self.crossfade_ms = ms;
                if self.crossfade_ms == 0 {
                    self.clear_fade();
                    if let Some(sink) = self.state.current_sink() {
                        sink.set_volume(self.state.volume());
                    }
                }
            }
            AudioCommand::ClearCache => {
                tracing::info!("用户触发：清除音频缓存");
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::ClearAll {
                        keep: self.state.path(),
                    })
                    .await;
            }
            AudioCommand::SetCacheBr(br) => {
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::PurgeNotBr {
                        br,
                        keep: self.state.path(),
                    })
                    .await;
            }
            AudioCommand::PrefetchAudio { id, br, url, title } => {
                tracing::info!(song_id = id, br, title = %title, "开始预缓存");
                let key = CacheKey { song_id: id, br };
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::EnsureCached {
                        token: 0,
                        key,
                        url,
                        title,
                        priority: Priority::Low,
                    })
                    .await;
            }
        }
    }

    fn start_playback(
        &mut self,
        key: &CacheKey,
        path: &std::path::Path,
        title: &str,
    ) -> Result<Option<u64>, String> {
        let (sink, duration_ms) = self.state.build_sink(path, None, title)?;
        let sink = Arc::new(sink);

        let has_current = self.state.current_sink().is_some();
        let can_fade = self.crossfade_ms > 0 && has_current && !self.state.paused();

        if can_fade {
            let old = self.state.take_current_for_fade();
            self.state.next_play_id();
            self.state.set_path(path.to_path_buf());
            sink.set_volume(0.0);
            sink.play();
            self.state.attach_sink(Arc::clone(&sink));
            if let Some(old) = old {
                old.set_volume(self.state.volume());
                self.fade = Some(Crossfade::new(old, Arc::clone(&sink), self.crossfade_ms));
                if let Some(fade) = &mut self.fade {
                    let _ = fade.apply(self.state.volume());
                }
            }
        } else {
            self.clear_fade();
            self.state.stop();
            self.state.set_path(path.to_path_buf());
            if self.state.paused() {
                sink.pause();
            } else {
                sink.play();
            }
            sink.set_volume(self.state.volume());
            self.state.attach_sink(Arc::clone(&sink));
        }

        tracing::debug!(song_id = key.song_id, path = %path.display(), "start playback");
        Ok(duration_ms)
    }
}

pub(super) fn spawn(
    rx_cmd: mpsc::Receiver<AudioCommand>,
    tx_evt: mpsc::Sender<AudioEvent>,
    data_dir: PathBuf,
    transfer_config: TransferConfig,
    settings: AudioSettings,
) {
    std::thread::spawn(move || {
        // 创建当前线程的 tokio runtime
        //
        // ## 为何使用 expect()？
        //
        // 1. **无法返回 Result**:
        //    - 此代码在 `std::thread::spawn` 闭包中运行
        //    - 闭包返回类型是 `()`，无法传播错误
        //    - 线程 panic 是唯一可行的失败处理方式
        //
        // 2. **实际风险极低**:
        //    - tokio runtime 创建只在以下情况失败：
        //      - 系统资源耗尽（内存、文件描述符）
        //      - 操作系统级别的配置错误
        //    - 这些情况下应用已无法正常运行
        //    - panic 是合理的系统级失败响应
        //
        // 3. **真正的风险已处理**:
        //    - `OutputStreamBuilder::open_default_stream()` 可能失败（无音频设备）
        //    - 已通过 `match` 处理，发送 `AudioEvent::Error` 给 UI
        //    - 用户会看到友好的错误消息
        //
        // 4. **未来改进方向**:
        //    - 考虑使用 `tokio::task::spawn_blocking` 替代 `std::thread::spawn`
        //    - 可以使用 `?` 传播错误到主线程
        //    - 但需要较大架构改动，且可能影响性能
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime: 系统资源不足或配置错误");
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, async move {
            let (tx_transfer, rx_transfer) =
                spawn_transfer_actor_with_config(data_dir.clone(), transfer_config);

            let stream = match OutputStreamBuilder::open_default_stream() {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(err = %e, "初始化音频输出失败");
                    let _ = tx_evt
                        .send(AudioEvent::Error(MessageError::other(format!(
                            "初始化音频输出失败: {e}"
                        ))))
                        .await;
                    return;
                }
            };
            let mixer = stream.mixer().clone();
            let state = PlayerState::new(mixer, stream);

            tracing::info!(data_dir = %data_dir.display(), "AudioWorker 已启动");

            let engine =
                AudioEngine::new(tx_evt, rx_cmd, tx_transfer, rx_transfer, state, settings);
            engine.run().await;
        });
    });
}
