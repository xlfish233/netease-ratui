use reqwest::blocking::Client;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::sync::mpsc;
use std::thread;
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum AudioCommand {
    PlayUrl { url: String, title: String },
    TogglePause,
    Stop,
}

#[derive(Debug)]
pub enum AudioEvent {
    NowPlaying { title: String, duration_ms: Option<u64> },
    Paused(bool),
    Stopped,
    Error(String),
}

pub fn spawn_audio_worker() -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<AudioCommand>();
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>();

    thread::spawn(move || {
        let http = Client::new();
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
                AudioCommand::PlayUrl { url, title } => match play_url(&http, &mut state, &url, &title) {
                    Ok(duration_ms) => {
                        let _ = tx_evt.send(AudioEvent::NowPlaying { title, duration_ms });
                    }
                    Err(e) => {
                        let _ = tx_evt.send(AudioEvent::Error(e));
                    }
                },
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
    sink: Option<Sink>,
    _temp: Option<NamedTempFile>,
}

impl PlayerState {
    fn new(handle: OutputStreamHandle) -> Self {
        Self {
            handle,
            sink: None,
            _temp: None,
        }
    }

    fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self._temp = None;
    }
}

fn play_url(
    http: &Client,
    state: &mut PlayerState,
    url: &str,
    title: &str,
) -> Result<Option<u64>, String> {
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

    state.sink = Some(sink);
    state._temp = Some(tmp);
    Ok(duration_ms)
}
