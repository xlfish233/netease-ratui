use rodio::mixer::Mixer;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub struct PlayerState {
    mixer: Mixer,
    #[allow(dead_code)]
    stream: OutputStream,
    current: Option<Arc<Sink>>,
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
            tracing::debug!(play_id = self.play_id, "Stopping current sink");
            cur.stop();
        }
    }

    pub fn take_current_for_fade(&mut self) -> Option<Arc<Sink>> {
        self.current.take()
    }

    pub fn current_sink(&self) -> Option<Arc<Sink>> {
        self.current.as_ref().map(Arc::clone)
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

    pub fn attach_sink(&mut self, sink: Arc<Sink>) {
        self.current = Some(sink);
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

    state.attach_sink(Arc::clone(&sink));
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
