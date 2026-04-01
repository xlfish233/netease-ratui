use crate::error::MessageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPlaybackMode {
    CachedFile,
    ProgressiveStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBufferState {
    Prebuffering,
    Buffering,
    Ready,
    Stalled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStreamHint {
    pub mode: AudioPlaybackMode,
    pub seekable: bool,
    pub buffer_state: AudioBufferState,
    pub buffered_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

impl AudioStreamHint {
    pub fn cached_file(total_bytes: Option<u64>) -> Self {
        Self {
            mode: AudioPlaybackMode::CachedFile,
            seekable: true,
            buffer_state: AudioBufferState::Ready,
            buffered_bytes: total_bytes,
            total_bytes,
        }
    }

    pub fn progressive(
        buffer_state: AudioBufferState,
        seekable: bool,
        buffered_bytes: u64,
        total_bytes: Option<u64>,
    ) -> Self {
        Self {
            mode: AudioPlaybackMode::ProgressiveStream,
            seekable,
            buffer_state,
            buffered_bytes: Some(buffered_bytes),
            total_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioLoadStage {
    CacheHit,
    DownloadQueued,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    PreparingPlayback,
    Retrying {
        attempt: u32,
        max_attempts: u32,
    },
}

#[derive(Debug)]
pub enum AudioCommand {
    PlayTrack {
        id: i64,
        br: i64,
        url: String,
        title: String,
        duration_ms: Option<u64>,
    },
    TogglePause,
    Stop,
    SeekToMs(u64),
    SetVolume(f32),
    SetCrossfadeMs(u64),
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
    Loading {
        song_id: i64,
        title: String,
        stage: AudioLoadStage,
        stream_hint: Option<AudioStreamHint>,
    },
    NowPlaying {
        song_id: i64,
        play_id: u64,
        title: String,
        duration_ms: Option<u64>,
        stream_hint: AudioStreamHint,
    },
    PlaybackHint {
        song_id: i64,
        play_id: u64,
        hint: AudioStreamHint,
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
    Error(MessageError),
    NeedsReload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_stream_hint_creation() {
        let hint = AudioStreamHint {
            mode: AudioPlaybackMode::ProgressiveStream,
            seekable: false,
            buffer_state: AudioBufferState::Prebuffering,
            buffered_bytes: Some(64 * 1024),
            total_bytes: Some(256 * 1024),
        };

        assert_eq!(hint.mode, AudioPlaybackMode::ProgressiveStream);
        assert!(!hint.seekable);
        assert_eq!(hint.buffer_state, AudioBufferState::Prebuffering);
        assert_eq!(hint.buffered_bytes, Some(64 * 1024));
    }

    #[test]
    fn test_audio_load_stage_debug_format() {
        let stage = AudioLoadStage::Downloading {
            downloaded_bytes: 1024,
            total_bytes: Some(2048),
        };
        let debug_str = format!("{:?}", stage);
        assert!(debug_str.contains("Downloading"));
    }

    #[test]
    fn test_audio_event_needs_reload_creation() {
        let event = AudioEvent::NeedsReload;
        match event {
            AudioEvent::NeedsReload => {
                // 测试可以成功创建和匹配 NeedsReload 事件
            }
            _ => panic!("Expected NeedsReload event"),
        }
    }

    #[test]
    fn test_audio_event_debug_format() {
        let event = AudioEvent::NeedsReload;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("NeedsReload"));
    }

    #[test]
    fn test_audio_event_loading_creation() {
        let event = AudioEvent::Loading {
            song_id: 1,
            title: "Test".to_owned(),
            stage: AudioLoadStage::CacheHit,
            stream_hint: None,
        };

        match event {
            AudioEvent::Loading {
                song_id,
                title,
                stage: AudioLoadStage::CacheHit,
                stream_hint: None,
            } => {
                assert_eq!(song_id, 1);
                assert_eq!(title, "Test");
            }
            other => panic!("Expected Loading(CacheHit), got {other:?}"),
        }
    }

    #[test]
    fn test_audio_event_playback_hint_creation() {
        let hint =
            AudioStreamHint::progressive(AudioBufferState::Buffering, false, 512, Some(1024));
        let event = AudioEvent::PlaybackHint {
            song_id: 7,
            play_id: 11,
            hint: hint.clone(),
        };

        match event {
            AudioEvent::PlaybackHint {
                song_id,
                play_id,
                hint: actual,
            } => {
                assert_eq!(song_id, 7);
                assert_eq!(play_id, 11);
                assert_eq!(actual, hint);
            }
            other => panic!("Expected PlaybackHint, got {other:?}"),
        }
    }

    #[test]
    fn test_audio_command_toggle_pause_creation() {
        let cmd = AudioCommand::TogglePause;
        match cmd {
            AudioCommand::TogglePause => {
                // 测试可以成功创建和匹配 TogglePause 命令
            }
            _ => panic!("Expected TogglePause command"),
        }
    }
}
