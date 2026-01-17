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
    NeedsReload,
}

#[cfg(test)]
mod tests {
    use super::*;

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
