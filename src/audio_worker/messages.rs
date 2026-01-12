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
    /// 设置“仅保留当前音质(br)”的缓存策略
    SetCacheBr(i64),
    /// 预缓存音频文件（仅缓存，不播放）
    PrefetchAudio {
        id: i64,
        br: i64,
        url: String,
        title: String,
    },
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
