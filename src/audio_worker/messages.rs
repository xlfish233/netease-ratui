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
