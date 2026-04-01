use crate::error::MessageError;

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
    },
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
    Error(MessageError),
    NeedsReload,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };

        match event {
            AudioEvent::Loading {
                song_id,
                title,
                stage: AudioLoadStage::CacheHit,
            } => {
                assert_eq!(song_id, 1);
                assert_eq!(title, "Test");
            }
            other => panic!("Expected Loading(CacheHit), got {other:?}"),
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
