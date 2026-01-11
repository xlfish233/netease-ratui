use reqwest::blocking::Client;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum AudioCommand {
    PlayTrack {
        id: i64,
        br: i64,
        url: String,
        title: String,
    },
    TogglePause,
    Stop,
    SeekToMs(u64),
    SetVolume(f32),
    ClearCache,
}

#[derive(Debug)]
pub enum AudioEvent {
    NowPlaying {
        song_id: i64,
        play_id: u64,
        title: String,
        duration_ms: Option<u64>,
    },
    Paused(bool),
    Stopped,
    Ended {
        play_id: u64,
    },
    CacheCleared {
        files: usize,
        bytes: u64,
    },
    Error(String),
}

pub fn spawn_audio_worker(
    data_dir: PathBuf,
) -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
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
        let cache = AudioCache::new(&data_dir);
        let mut state = PlayerState::new(handle, cache);

        while let Ok(cmd) = rx_cmd.recv() {
            match cmd {
                AudioCommand::PlayTrack { id, br, url, title } => {
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
                    let _ = tx_evt.send(AudioEvent::CacheCleared { files, bytes });
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}

struct PlayerState {
    handle: OutputStreamHandle,
    sink: Option<Arc<Sink>>,
    temp: Option<NamedTempFile>,
    path: Option<PathBuf>,
    end_cancel: Option<Arc<AtomicBool>>,
    play_id: u64,
    paused: bool,
    volume: f32,
    cache: AudioCache,
}

impl PlayerState {
    fn new(handle: OutputStreamHandle, cache: AudioCache) -> Self {
        Self {
            handle,
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

    fn stop(&mut self) {
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

fn play_track(
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

    let (sink, duration_ms) = match build_sink_from_path(&state.handle, &path, None, title) {
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
                build_sink_from_path(&state.handle, &path, None, title)?
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

fn seek_to_ms(
    tx_evt: &mpsc::Sender<AudioEvent>,
    state: &mut PlayerState,
    position_ms: u64,
) -> Result<(), String> {
    let Some(path) = state.path.clone() else {
        return Ok(());
    };

    state.restart_keep_play_id();

    let seek = Duration::from_millis(position_ms);
    let (sink, _duration_ms) = build_sink_from_path(&state.handle, &path, Some(seek), "seek")?;
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
    handle: &OutputStreamHandle,
    path: &Path,
    seek: Option<Duration>,
    title: &str,
) -> Result<(Sink, Option<u64>), String> {
    let file = File::open(path).map_err(|e| format!("打开音频文件失败({title}): {e}"))?;
    let decoder =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("解码失败({title}): {e}"))?;
    let duration_ms = decoder.total_duration().map(|d| d.as_millis() as u64);
    let source: Box<dyn Source<Item = i16> + Send> = if let Some(seek) = seek {
        Box::new(decoder.skip_duration(seek))
    } else {
        Box::new(decoder)
    };

    let sink = Sink::try_new(handle).map_err(|e| format!("创建 Sink 失败: {e}"))?;
    sink.append(source);
    Ok((sink, duration_ms))
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct CacheIndex {
    #[serde(default)]
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    file_name: String,
    size_bytes: u64,
    last_access_ms: u64,
}

enum ResolvedAudio {
    Path(PathBuf),
    Temp(NamedTempFile),
}

struct AudioCache {
    dir: Option<PathBuf>,
    index_path: Option<PathBuf>,
    index: CacheIndex,
    max_bytes: u64,
}

impl AudioCache {
    fn new(data_dir: &Path) -> Self {
        const INDEX_VERSION: u32 = 2;

        let max_bytes = env::var("NETEASE_AUDIO_CACHE_MAX_MB")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2048)
            .saturating_mul(1024)
            .saturating_mul(1024);

        let dir = data_dir.join("audio_cache");
        if fs::create_dir_all(&dir).is_err() {
            return Self {
                dir: None,
                index_path: None,
                index: CacheIndex::default(),
                max_bytes,
            };
        }

        let index_path = dir.join("index.json");
        let mut index = fs::read(&index_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<CacheIndex>(&b).ok())
            .unwrap_or_default();

        if index.version != INDEX_VERSION {
            // 废弃旧索引/旧命名规则：直接清空缓存目录
            let _ = clear_dir_files(&dir, None);
            index = CacheIndex {
                version: INDEX_VERSION,
                entries: HashMap::new(),
            };
            let bytes = serde_json::to_vec_pretty(&index).unwrap_or_default();
            let _ = fs::write(&index_path, bytes);
        }

        Self {
            dir: Some(dir),
            index_path: Some(index_path),
            index,
            max_bytes,
        }
    }

    fn resolve_audio_file(
        &mut self,
        http: &Client,
        song_id: i64,
        br: i64,
        url: &str,
        title: &str,
    ) -> Result<ResolvedAudio, String> {
        let Some(dir) = self.dir.as_ref() else {
            // fallback: no cache dir
            let mut tmp = NamedTempFile::new().map_err(|e| format!("创建临时文件失败: {e}"))?;
            download_to_file(http, &mut tmp, url, title)?;
            return Ok(ResolvedAudio::Temp(tmp));
        };

        let key = format!("{song_id}_{br}");
        let file_name = format!("{key}.bin");
        let path = dir.join(&file_name);

        if path.exists() {
            self.touch(&key, &file_name, &path);
            self.persist_index();
            return Ok(ResolvedAudio::Path(path));
        }

        let mut tmp = NamedTempFile::new_in(dir)
            .map_err(|e| format!("创建缓存临时文件失败({title}): {e}"))?;
        download_to_file(http, &mut tmp, url, title)?;

        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        tmp.persist(&path)
            .map_err(|e| format!("写入缓存文件失败({title}): {e}"))?;

        self.touch(&key, &file_name, &path);
        self.cleanup(Some(&path));
        self.persist_index();

        Ok(ResolvedAudio::Path(path))
    }

    fn touch(&mut self, key: &str, file_name: &str, path: &Path) {
        let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        self.index.entries.insert(
            key.to_owned(),
            CacheEntry {
                file_name: file_name.to_owned(),
                size_bytes,
                last_access_ms: now_ms(),
            },
        );
    }

    fn cleanup(&mut self, keep: Option<&Path>) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };

        // remove missing
        self.index
            .entries
            .retain(|_, ent| dir.join(&ent.file_name).exists());

        let mut total: u64 = self.index.entries.values().map(|e| e.size_bytes).sum();
        if total <= self.max_bytes {
            return;
        }

        let mut entries = self
            .index
            .entries
            .iter()
            .map(|(k, v)| {
                (
                    k.to_owned(),
                    v.last_access_ms,
                    v.file_name.clone(),
                    v.size_bytes,
                )
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(_, ts, _, _)| *ts);

        for (k, _ts, file_name, size) in entries {
            if total <= self.max_bytes {
                break;
            }
            let p = dir.join(&file_name);
            if keep.is_some_and(|kp| kp == p.as_path()) {
                continue;
            }
            let _ = fs::remove_file(&p);
            self.index.entries.remove(&k);
            total = total.saturating_sub(size);
        }
    }

    fn persist_index(&self) {
        let (Some(dir), Some(index_path)) = (self.dir.as_ref(), self.index_path.as_ref()) else {
            return;
        };
        let bytes = match serde_json::to_vec_pretty(&self.index) {
            Ok(b) => b,
            Err(_) => return,
        };
        let mut tmp = match NamedTempFile::new_in(dir) {
            Ok(f) => f,
            Err(_) => return,
        };
        if tmp.write_all(&bytes).is_err() {
            return;
        }
        let _ = tmp.persist(index_path);
    }

    fn invalidate(&mut self, song_id: i64, br: i64) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };
        let key = format!("{song_id}_{br}");
        if let Some(ent) = self.index.entries.remove(&key) {
            let _ = fs::remove_file(dir.join(ent.file_name));
        } else {
            let _ = fs::remove_file(dir.join(format!("{key}.bin")));
        }
        self.persist_index();
    }

    fn clear_all(&mut self, keep: Option<&Path>) -> (usize, u64) {
        let Some(dir) = self.dir.as_ref() else {
            return (0, 0);
        };

        let (files, bytes) = clear_dir_files(dir, keep);
        self.index.entries.clear();
        self.persist_index();
        (files, bytes)
    }
}

fn clear_dir_files(dir: &Path, keep: Option<&Path>) -> (usize, u64) {
    let mut removed_files = 0usize;
    let mut removed_bytes = 0u64;

    let keep = keep.and_then(|p| p.file_name().map(|n| dir.join(n)));

    let Ok(rd) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            continue;
        }
        if p.file_name().is_some_and(|n| n == "index.json") {
            continue;
        }
        if keep.as_ref().is_some_and(|kp| kp == &p) {
            continue;
        }

        if let Ok(md) = ent.metadata() {
            removed_bytes = removed_bytes.saturating_add(md.len());
        }
        if fs::remove_file(&p).is_ok() {
            removed_files += 1;
        }
    }

    (removed_files, removed_bytes)
}

fn download_to_file(
    http: &Client,
    out: &mut NamedTempFile,
    url: &str,
    title: &str,
) -> Result<(), String> {
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
        out.write_all(&buf[..n])
            .map_err(|e| format!("写入临时文件失败({title}): {e}"))?;
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
