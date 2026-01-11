use reqwest::blocking::Client;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum AudioCommand {
    PlayUrl { url: String, title: String },
    TogglePause,
    Stop,
}

#[derive(Debug)]
pub enum AudioEvent {
    NowPlaying {
        play_id: u64,
        title: String,
        duration_ms: Option<u64>,
    },
    Paused(bool),
    Stopped,
    Ended {
        play_id: u64,
    },
    Error(String),
}

pub fn spawn_audio_worker() -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<AudioCommand>();
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>();

    thread::spawn(move || {
        let http = match Client::builder().timeout(Duration::from_secs(30)).build() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx_evt.send(AudioEvent::Error(format!("初始化 HTTP 客户端失败: {e}")));
                return;
            }
        };
        let (stream, handle) = match OutputStream::try_default() {
            Ok(v) => v,
            Err(e) => {
                let _ = tx_evt.send(AudioEvent::Error(format!("初始化音频输出失败: {e}")));
                return;
            }
        };

        let _stream_guard = stream;
        let mut state = PlayerState::new(handle);

        while let Ok(cmd) = rx_cmd.recv() {
            match cmd {
                AudioCommand::PlayUrl { url, title } => {
                    match play_url(&http, &tx_evt, &mut state, &url, &title) {
                        Ok((play_id, duration_ms)) => {
                            let _ = tx_evt.send(AudioEvent::NowPlaying {
                                play_id,
                                title,
                                duration_ms,
                            });
                        }
                        Err(e) => {
                            let _ = tx_evt.send(AudioEvent::Error(e));
                        }
                    }
                }
                AudioCommand::TogglePause => {
                    if let Some(sink) = state.sink.as_ref() {
                        if sink.is_paused() {
                            sink.play();
                            let _ = tx_evt.send(AudioEvent::Paused(false));
                        } else {
                            sink.pause();
                            let _ = tx_evt.send(AudioEvent::Paused(true));
                        }
                    }
                }
                AudioCommand::Stop => {
                    state.stop();
                    let _ = tx_evt.send(AudioEvent::Stopped);
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}

struct PlayerState {
    handle: OutputStreamHandle,
    sink: Option<Arc<Sink>>,
    _temp: Option<NamedTempFile>,
    end_cancel: Option<Arc<AtomicBool>>,
    play_id: u64,
}

impl PlayerState {
    fn new(handle: OutputStreamHandle) -> Self {
        Self {
            handle,
            sink: None,
            _temp: None,
            end_cancel: None,
            play_id: 0,
        }
    }

    fn stop(&mut self) {
        self.play_id = self.play_id.wrapping_add(1);
        if let Some(c) = self.end_cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self._temp = None;
    }
}

fn play_url(
    http: &Client,
    tx_evt: &mpsc::Sender<AudioEvent>,
    state: &mut PlayerState,
    url: &str,
    title: &str,
) -> Result<(u64, Option<u64>), String> {
    state.stop();

    let mut tmp = NamedTempFile::new().map_err(|e| format!("创建临时文件失败: {e}"))?;
    let path = tmp.path().to_path_buf();

    let mut resp = http
        .get(url)
        .send()
        .map_err(|e| format!("下载音频失败({title}): {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载音频失败({title}): HTTP {}", resp.status()));
    }

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("下载音频失败({title}): {e}"))?;
        if n == 0 {
            break;
        }
        tmp.write_all(&buf[..n])
            .map_err(|e| format!("写入临时文件失败({title}): {e}"))?;
    }

    let file = File::open(&path).map_err(|e| format!("打开临时文件失败({title}): {e}"))?;
    let source =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("解码失败({title}): {e}"))?;
    let duration_ms = source
        .total_duration()
        .map(|d: std::time::Duration| d.as_millis() as u64);

    let sink = Sink::try_new(&state.handle).map_err(|e| format!("创建 Sink 失败: {e}"))?;
    sink.append(source);
    sink.play();
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
    state._temp = Some(tmp);
    Ok((play_id, duration_ms))
}
