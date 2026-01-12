use reqwest::Client;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::sync::mpsc;

use super::cache::AudioCache;
use super::download::{download_to_path_with_config, now_ms};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub song_id: i64,
    pub br: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Low,
}

impl Priority {
    fn as_u8(self) -> u8 {
        match self {
            Priority::High => 1,
            Priority::Low => 0,
        }
    }
}

#[derive(Debug)]
pub enum TransferCommand {
    /// Ensure this key is cached; token==0 means "no callback" (fire-and-forget).
    EnsureCached {
        token: u64,
        key: CacheKey,
        url: String,
        title: String,
        priority: Priority,
    },
    /// Remove a specific cached entry.
    Invalidate {
        key: CacheKey,
    },
    ClearAll {
        keep: Option<PathBuf>,
    },
    /// Keep only this bitrate in cache (best-effort).
    PurgeNotBr {
        br: i64,
        keep: Option<PathBuf>,
    },
}

#[derive(Debug)]
pub enum TransferEvent {
    Ready {
        token: u64,
        key: CacheKey,
        path: PathBuf,
    },
    Error {
        token: u64,
        message: String,
    },
    CacheCleared {
        files: usize,
        bytes: u64,
    },
}

#[derive(Debug, Clone, Copy)]
struct HeapItem {
    prio: u8,
    seq: u64,
    key: CacheKey,
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher prio first; then FIFO by seq (lower seq first).
        self.prio
            .cmp(&other.prio)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.prio == other.prio && self.seq == other.seq && self.key == other.key
    }
}

impl Eq for HeapItem {}

struct JobState {
    waiters: Vec<u64>,
    url: String,
    title: String,
    prio: u8,
    in_flight: bool,
}

#[derive(Debug)]
enum JobResult {
    Ok { key: CacheKey, tmp_path: PathBuf },
    Err { key: CacheKey, message: String },
}

pub type TransferSender = mpsc::Sender<TransferCommand>;
pub type TransferReceiver = mpsc::Receiver<TransferEvent>;

/// 传输配置
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// HTTP 超时（秒）
    pub http_timeout_secs: u64,
    /// HTTP 连接超时（秒）
    pub http_connect_timeout_secs: u64,
    /// 下载并发数（None 表示自动检测）
    pub download_concurrency: Option<usize>,
    /// 下载重试次数
    pub download_retries: u32,
    /// 重试退避初始时间（毫秒）
    pub download_retry_backoff_ms: u64,
    /// 重试退避最大时间（毫秒）
    pub download_retry_backoff_max_ms: u64,
    /// 音频缓存大小（MB）
    pub audio_cache_max_mb: usize,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            http_timeout_secs: env::var("NETEASE_AUDIO_HTTP_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            http_connect_timeout_secs: env::var("NETEASE_AUDIO_HTTP_CONNECT_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            download_concurrency: env::var("NETEASE_AUDIO_DOWNLOAD_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|v| *v > 0),
            download_retries: env::var("NETEASE_AUDIO_DOWNLOAD_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            download_retry_backoff_ms: env::var("NETEASE_AUDIO_DOWNLOAD_RETRY_BACKOFF_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(250),
            download_retry_backoff_max_ms: env::var("NETEASE_AUDIO_DOWNLOAD_RETRY_BACKOFF_MAX_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2_000),
            audio_cache_max_mb: env::var("NETEASE_AUDIO_CACHE_MAX_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2048),
        }
    }
}

pub fn spawn_transfer_actor_with_config(
    data_dir: PathBuf,
    config: TransferConfig,
) -> (TransferSender, TransferReceiver) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<TransferCommand>(256);
    let (tx_evt, rx_evt) = mpsc::channel::<TransferEvent>(256);

    let run = async move {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.http_timeout_secs))
            .connect_timeout(Duration::from_secs(config.http_connect_timeout_secs))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(err = %e, "初始化 HTTP 客户端失败");
                Client::new()
            });

        let concurrency = config.download_concurrency.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        });

        let semaphore = Arc::new(Semaphore::new(concurrency));
        tracing::info!(
            concurrency,
            timeout_secs = config.http_timeout_secs,
            connect_timeout_secs = config.http_connect_timeout_secs,
            retries = config.download_retries,
            cache_max_mb = config.audio_cache_max_mb,
            "TransferActor 已启动（配置化模式）"
        );

        let mut cache = AudioCache::new_with_config(&data_dir, config.audio_cache_max_mb);
        let cache_dir = cache.cache_dir().map(|p| p.to_path_buf());

        let (tx_done, mut rx_done) = mpsc::channel::<JobResult>(256);

        let mut seq: u64 = 1;
        let mut heap = BinaryHeap::<HeapItem>::new();
        let mut jobs = HashMap::<CacheKey, JobState>::new();
        let mut active_br: i64 = 0;
        let mut tmp_seq: u64 = 1;

        let mut rx_cmd = rx_cmd;
        loop {
            tokio::select! {
                Some(cmd) = rx_cmd.recv() => {
                    match cmd {
                        TransferCommand::EnsureCached { token, key, url, title, priority } => {
                            // Fast path: cache hit.
                            if let Some(path) = cache.lookup_path(key.song_id, key.br) {
                                if token != 0 {
                                    let _ = tx_evt.send(TransferEvent::Ready { token, key, path }).await;
                                }
                                continue;
                            }

                            let st = jobs.entry(key).or_insert(JobState {
                                waiters: Vec::new(),
                                url: url.clone(),
                                title: title.clone(),
                                prio: priority.as_u8(),
                                in_flight: false,
                            });
                            st.url = url;
                            st.title = title;
                            st.prio = st.prio.max(priority.as_u8());
                            st.waiters.push(token);

                            if !st.in_flight {
                                heap.push(HeapItem { prio: st.prio, seq, key });
                                seq = seq.wrapping_add(1);
                            }
                        }
                        TransferCommand::Invalidate { key } => {
                            cache.invalidate(key.song_id, key.br);
                        }
                        TransferCommand::ClearAll { keep } => {
                            let (files, bytes) = cache.clear_all(keep.as_deref());
                            let _ = tx_evt.send(TransferEvent::CacheCleared { files, bytes }).await;
                        }
                        TransferCommand::PurgeNotBr { br, keep } => {
                            active_br = br;
                            cache.purge_not_br(br, keep.as_deref());
                        }
                    }
                }
                Some(done) = rx_done.recv() => {
                    match done {
                        JobResult::Ok { key, tmp_path } => {
                            let final_path = match cache.commit_tmp_file(key.song_id, key.br, &tmp_path) {
                                Ok(p) => p,
                                Err(e) => {
                                    let _ = tokio::fs::remove_file(&tmp_path).await;
                                    // Fan out errors to waiters.
                                    if let Some(st) = jobs.remove(&key) {
                                        for token in st.waiters.into_iter().filter(|t| *t != 0) {
                                            let _ = tx_evt.send(TransferEvent::Error { token, message: e.clone() }).await;
                                        }
                                    }
                                    continue;
                                }
                            };

                            // Enforce "only keep current br" policy (best-effort).
                            if active_br != 0 && key.br == active_br {
                                cache.purge_song_other_brs(key.song_id, key.br, None);
                            } else if active_br != 0 && key.br != active_br {
                                cache.purge_not_br(active_br, None);
                            } else {
                                cache.purge_song_other_brs(key.song_id, key.br, None);
                            }

                            if let Some(st) = jobs.remove(&key) {
                                for token in st.waiters.into_iter().filter(|t| *t != 0) {
                                    let _ = tx_evt.send(TransferEvent::Ready { token, key, path: final_path.clone() }).await;
                                }
                            }
                        }
                        JobResult::Err { key, message } => {
                            if let Some(st) = jobs.remove(&key) {
                                for token in st.waiters.into_iter().filter(|t| *t != 0) {
                                    let _ = tx_evt.send(TransferEvent::Error { token, message: message.clone() }).await;
                                }
                            }
                        }
                    }
                }
                else => break,
            }

            // Try to start as many jobs as possible (bounded by semaphore permits).
            loop {
                let permit = match semaphore.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => break,
                };

                // Pick next highest-priority queued job.
                let mut next = None::<CacheKey>;
                while let Some(item) = heap.pop() {
                    let key = item.key;
                    let Some(st) = jobs.get(&key) else {
                        continue;
                    };
                    if st.in_flight || st.prio != item.prio {
                        continue;
                    }
                    next = Some(key);
                    break;
                }

                let Some(key) = next else {
                    drop(permit);
                    break;
                };

                // Mark in-flight and spawn download task.
                let Some(st) = jobs.get_mut(&key) else {
                    drop(permit);
                    continue;
                };
                st.in_flight = true;

                let Some(dir) = cache_dir.as_ref() else {
                    st.in_flight = false;
                    let message = "缓存目录不可用".to_owned();
                    let waiters = st.waiters.clone();
                    jobs.remove(&key);
                    drop(permit);
                    for token in waiters.into_iter().filter(|t| *t != 0) {
                        let _ = tx_evt
                            .send(TransferEvent::Error {
                                token,
                                message: message.clone(),
                            })
                            .await;
                    }
                    continue;
                };

                let tmp_path = tmp_path_for(dir, key, tmp_seq);
                tmp_seq = tmp_seq.wrapping_add(1);

                let url = st.url.clone();
                let title = st.title.clone();
                let http = http.clone();
                let tx_done = tx_done.clone();
                let retries = config.download_retries;
                let backoff_ms = config.download_retry_backoff_ms;
                let backoff_max_ms = config.download_retry_backoff_max_ms;

                tokio::spawn(async move {
                    let _permit = permit;
                    let res = download_to_path_with_config(
                        &http,
                        &tmp_path,
                        &url,
                        &title,
                        retries,
                        backoff_ms,
                        backoff_max_ms,
                    )
                    .await;
                    match res {
                        Ok(_) => {
                            let _ = tx_done.send(JobResult::Ok { key, tmp_path }).await;
                        }
                        Err(e) => {
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            let _ = tx_done.send(JobResult::Err { key, message: e }).await;
                        }
                    }
                });
            }
        }
    };

    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(run);
    } else {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(run);
        });
    }

    (tx_cmd, rx_evt)
}

fn tmp_path_for(dir: &Path, key: CacheKey, seq: u64) -> PathBuf {
    dir.join(format!(
        "{}_{}.{}.{}.tmp",
        key.song_id,
        key.br,
        now_ms(),
        seq,
    ))
}
