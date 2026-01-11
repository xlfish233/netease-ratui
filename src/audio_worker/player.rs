use reqwest::blocking::Client;
use rodio::mixer::Mixer;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

use super::cache::{AudioCache, ResolvedAudio};
use super::messages::AudioEvent;

pub struct PlayerState {
    mixer: Mixer,
    #[allow(dead_code)]
    stream: OutputStream,
    pub(super) sink: Option<Arc<Sink>>,
    temp: Option<NamedTempFile>,
    pub(super) path: Option<PathBuf>,
    end_cancel: Option<Arc<AtomicBool>>,
    play_id: u64,
    pub(super) paused: bool,
    pub(super) volume: f32,
    pub cache: AudioCache,
}

impl PlayerState {
    pub fn new(mixer: Mixer, stream: OutputStream, cache: AudioCache) -> Self {
        Self {
            mixer,
            stream,
            sink: None,
            temp: None,
            path: None,
            end_cancel: None,
            play_id: 0,
            paused: false,
            volume: 1.0,
            cache,
        }
    }

    pub fn stop(&mut self) {
        self.play_id = self.play_id.wrapping_add(1);
        if let Some(c) = self.end_cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self.temp = None;
        self.path = None;
    }

    fn restart_keep_play_id(&mut self) {
        if let Some(c) = self.end_cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }
}

pub(super) fn play_track(
    http: &Client,
    tx_evt: &mpsc::Sender<AudioEvent>,
    state: &mut PlayerState,
    song_id: i64,
    br: i64,
    url: &str,
    title: &str,
) -> Result<(u64, Option<u64>), String> {
    state.stop();

    let resolved = state
        .cache
        .resolve_audio_file(http, song_id, br, url, title)?;
    let is_cached = matches!(resolved, ResolvedAudio::Path(_));
    let path = match resolved {
        ResolvedAudio::Path(path) => {
            state.temp = None;
            path
        }
        ResolvedAudio::Temp(tmp) => {
            let path = tmp.path().to_path_buf();
            state.temp = Some(tmp);
            path
        }
    };
    state.path = Some(path.clone());

    let (sink, duration_ms) = match build_sink_from_path(&state.mixer, &path, None, title) {
        Ok(v) => v,
        Err(e) => {
            // 如果是缓存文件损坏/解码失败，清掉缓存并重试一次下载
            if is_cached {
                state.cache.invalidate(song_id, br);
                let resolved = state
                    .cache
                    .resolve_audio_file(http, song_id, br, url, title)?;
                let path = match resolved {
                    ResolvedAudio::Path(path) => {
                        state.temp = None;
                        path
                    }
                    ResolvedAudio::Temp(tmp) => {
                        let path = tmp.path().to_path_buf();
                        state.temp = Some(tmp);
                        path
                    }
                };
                state.path = Some(path.clone());
                build_sink_from_path(&state.mixer, &path, None, title)?
            } else {
                return Err(e);
            }
        }
    };
    sink.set_volume(state.volume);
    if state.paused {
        sink.pause();
    } else {
        sink.play();
    }
    let sink = Arc::new(sink);

    let play_id = state.play_id;
    let tx_end = tx_evt.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.end_cancel = Some(Arc::clone(&cancel));
    let sink_end = Arc::clone(&sink);
    thread::spawn(move || {
        sink_end.sleep_until_end();
        if !cancel.load(Ordering::Relaxed) {
            let _ = tx_end.send(AudioEvent::Ended { play_id });
        }
    });

    state.sink = Some(sink);
    Ok((play_id, duration_ms))
}

pub(super) fn seek_to_ms(
    tx_evt: &mpsc::Sender<AudioEvent>,
    state: &mut PlayerState,
    position_ms: u64,
) -> Result<(), String> {
    let Some(path) = state.path.clone() else {
        return Ok(());
    };

    state.restart_keep_play_id();

    let seek = Duration::from_millis(position_ms);
    let (sink, _duration_ms) = build_sink_from_path(&state.mixer, &path, Some(seek), "seek")?;
    sink.set_volume(state.volume);
    if state.paused {
        sink.pause();
    } else {
        sink.play();
    }
    let sink = Arc::new(sink);

    let play_id = state.play_id;
    let tx_end = tx_evt.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.end_cancel = Some(Arc::clone(&cancel));
    let sink_end = Arc::clone(&sink);
    thread::spawn(move || {
        sink_end.sleep_until_end();
        if !cancel.load(Ordering::Relaxed) {
            let _ = tx_end.send(AudioEvent::Ended { play_id });
        }
    });

    state.sink = Some(sink);
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
