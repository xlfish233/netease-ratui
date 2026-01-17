use rodio::mixer::Mixer;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

use super::messages::AudioEvent;

struct ActiveSink {
    sink: Arc<Sink>,
    end_cancel: Arc<AtomicBool>,
}

pub struct PlayerState {
    mixer: Mixer,
    #[allow(dead_code)]
    stream: OutputStream,
    current: Option<ActiveSink>,
    path: Option<PathBuf>,
    play_id: u64,
    paused: bool,
    volume: f32,
}

impl PlayerState {
    pub fn new(mixer: Mixer, stream: OutputStream) -> Self {
        Self {
            mixer,
            stream,
            current: None,
            path: None,
            play_id: 0,
            paused: false,
            volume: 1.0,
        }
    }

    pub fn play_id(&self) -> u64 {
        self.play_id
    }

    pub fn next_play_id(&mut self) -> u64 {
        self.play_id = self.play_id.wrapping_add(1).max(1);
        self.play_id
    }

    pub fn stop(&mut self) {
        self.play_id = self.play_id.wrapping_add(1).max(1);
        self.stop_current();
        self.path = None;
    }

    pub fn stop_keep_play_id(&mut self) {
        self.stop_current();
    }

    fn stop_current(&mut self) {
        if let Some(cur) = self.current.take() {
            tracing::debug!(
                play_id = self.play_id,
                "Stopping current sink, signaling end check thread to cancel"
            );
            cur.end_cancel.store(true, Ordering::Relaxed);
            cur.sink.stop();
        }
    }

    pub fn cancel_current_end(&mut self) {
        if let Some(cur) = self.current.as_ref() {
            cur.end_cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn take_current_for_fade(&mut self) -> Option<Arc<Sink>> {
        if let Some(cur) = self.current.take() {
            cur.end_cancel.store(true, Ordering::Relaxed);
            Some(cur.sink)
        } else {
            None
        }
    }

    pub fn current_sink(&self) -> Option<Arc<Sink>> {
        self.current.as_ref().map(|cur| Arc::clone(&cur.sink))
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn attach_sink(&mut self, tx_evt: &mpsc::Sender<AudioEvent>, sink: Arc<Sink>) {
        let play_id = self.play_id;
        let tx_end = tx_evt.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let sink_end = Arc::clone(&sink);
        let cancel_end = Arc::clone(&cancel);

        let thread_name = format!("audio-end-check-{}", play_id);
        tracing::debug!(play_id, "Spawning end check thread");

        // 启动后台线程监控播放结束
        //
        // ## 为何使用 expect()？
        //
        // 1. **功能降级而非崩溃**:
        //    - 如果线程创建失败，`end_cancel` 不会被触发
        //    - 播放仍可继续，只是无法自动检测歌曲结束
        //    - 不影响核心播放功能
        //
        // 2. **实际风险**:
        //    - 系统资源严重不足时可能失败
        //    - 极端情况下，但不应导致整个应用 panic
        //
        // 3. **未来改进方向**:
        //    - 改为返回 `Result<(), AudioError>`
        //    - 在调用方决定如何处理（降级或报错）
        //    - 需要修改 `attach_sink` 签名
        //
        // ## 当前权衡:
        // - 简单性: 避免复杂的错误传播
        // - 影响: 有限，只影响自动切歌
        thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let start = std::time::Instant::now();
                sink_end.sleep_until_end();
                let elapsed = start.elapsed();

                if !cancel_end.load(Ordering::Relaxed) {
                    tracing::debug!(
                        play_id,
                        elapsed_ms = elapsed.as_millis(),
                        "End check thread exiting naturally"
                    );
                    let _ = tx_end.blocking_send(AudioEvent::Ended { play_id });
                } else {
                    tracing::debug!(
                        play_id,
                        elapsed_ms = elapsed.as_millis(),
                        "End check thread was cancelled"
                    );
                }
            })
            .expect("failed to spawn end check thread: 系统资源不足");

        self.current = Some(ActiveSink {
            sink,
            end_cancel: cancel,
        });
    }

    pub fn build_sink(
        &self,
        path: &Path,
        seek: Option<Duration>,
        title: &str,
    ) -> Result<(Sink, Option<u64>), String> {
        build_sink_from_path(&self.mixer, path, seek, title)
    }
}

pub(super) fn seek_to_ms(
    tx_evt: &mpsc::Sender<AudioEvent>,
    state: &mut PlayerState,
    position_ms: u64,
) -> Result<(), String> {
    let Some(path) = state.path() else {
        return Ok(());
    };

    state.stop_keep_play_id();

    let seek = Duration::from_millis(position_ms);
    let (sink, _duration_ms) = state.build_sink(&path, Some(seek), "seek")?;
    let sink = Arc::new(sink);
    sink.set_volume(state.volume());
    if state.paused() {
        sink.pause();
    } else {
        sink.play();
    }

    state.attach_sink(tx_evt, Arc::clone(&sink));
    Ok(())
}

fn build_sink_from_path(
    mixer: &Mixer,
    path: &Path,
    seek: Option<Duration>,
    title: &str,
) -> Result<(Sink, Option<u64>), String> {
    let file = File::open(path).map_err(|e| format!("打开音频文件失败({title}): {e}"))?;
    let decoder =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("解码失败({title}): {e}"))?;
    let duration_ms = decoder.total_duration().map(|d| d.as_millis() as u64);
    let source: Box<dyn Source + Send> = if let Some(seek) = seek {
        Box::new(decoder.skip_duration(seek))
    } else {
        Box::new(decoder)
    };

    let sink = Sink::connect_new(mixer);
    sink.append(source);
    Ok((sink, duration_ms))
}
