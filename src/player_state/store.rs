use crate::app::state::{App, PlayMode};
use crate::app::{PlayQueue, PlaylistPreload};
use crate::domain::model::{Playlist, Song};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CURRENT_VERSION: u8 = 3;
const STATE_FILE: &str = "player_state.json";

/// 轻量级歌曲信息（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongLite {
    pub id: i64,
    pub name: String,
    pub artists: String,
}

impl From<&Song> for SongLite {
    fn from(song: &Song) -> Self {
        Self {
            id: song.id,
            name: song.name.clone(),
            artists: song.artists.clone(),
        }
    }
}

/// 可序列化的播放队列状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayQueueState {
    pub songs: Vec<SongLite>,
    pub order: Vec<usize>,
    pub cursor: Option<usize>,
    pub mode: String,
}

/// 播放进度（使用时间戳替代 Instant）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackProgress {
    pub started_at_epoch_ms: Option<i64>,
    pub total_ms: Option<u64>,
    pub paused: bool,
    pub paused_at_epoch_ms: Option<i64>,
    pub paused_accum_ms: u64,
}

/// 播放器状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub version: u8,
    pub play_song_id: Option<i64>,
    pub progress: PlaybackProgress,
    pub play_queue: PlayQueueState,
    pub volume: f32,
    pub play_br: i64,
    pub crossfade_ms: u64,
}

/// 轻量级歌单信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistLite {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub special_type: i64,
}

impl From<&Playlist> for PlaylistLite {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name.clone(),
            track_count: playlist.track_count,
            special_type: playlist.special_type,
        }
    }
}

/// 完整应用状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub version: u8,
    pub player: PlayerState,
    pub playlists: Vec<PlaylistLite>,
    pub playlists_selected: usize,
    #[serde(default)]
    pub playlist_preloads: HashMap<i64, PlaylistPreload>,
    pub saved_at_epoch_ms: i64,
}

/// 错误类型
#[derive(Debug)]
pub enum PlayerStateError {
    Io(std::io::Error),
    Serde(serde_json::Error),
    IncompatibleVersion { expected: u8, found: u8 },
}

impl std::fmt::Display for PlayerStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerStateError::Io(e) => write!(f, "IO 错误: {}", e),
            PlayerStateError::Serde(e) => write!(f, "序列化错误: {}", e),
            PlayerStateError::IncompatibleVersion { expected, found } => {
                write!(f, "版本不兼容: 预期 {}, 找到 {}", expected, found)
            }
        }
    }
}

impl std::error::Error for PlayerStateError {}

/// 计算播放进度（毫秒）
fn playback_elapsed_ms(app: &App) -> u64 {
    if let Some(started) = app.play_started_at {
        let elapsed = started.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        // 如果当前暂停，需要减去暂停累积时间
        if app.paused {
            elapsed_ms.saturating_sub(app.play_paused_accum_ms)
        } else {
            elapsed_ms
        }
    } else {
        0
    }
}

/// 将 App 转换为持久化格式
fn app_to_snapshot(app: &App) -> AppStateSnapshot {
    let now = chrono::Utc::now().timestamp_millis();

    // 计算播放进度
    let elapsed_ms = playback_elapsed_ms(app);

    // 反推 started_at 时间戳：saved_at - elapsed = started_at
    let started_at_epoch_ms = if elapsed_ms > 0 {
        Some(now - elapsed_ms as i64)
    } else {
        None
    };

    // 计算暂停累积时间的时间戳
    let paused_at_epoch_ms = if app.paused {
        if let Some(paused_at) = app.play_paused_at {
            // paused_at 是 Instant，需要转换为时间戳
            // paused_at_epoch_ms = now - (now - paused_at)
            Some(now - paused_at.elapsed().as_millis() as i64)
        } else {
            Some(now)
        }
    } else {
        None
    };

    // 转换播放队列
    let play_queue = PlayQueueState {
        songs: app.play_queue.songs().iter().map(SongLite::from).collect(),
        order: app
            .play_queue
            .ordered_songs()
            .iter()
            .filter_map(|s| app.play_queue.songs().iter().position(|x| x.id == s.id))
            .collect(),
        cursor: app.play_queue.cursor_pos(),
        mode: play_mode_to_string(app.play_mode),
    };

    // 转换歌单
    let playlists: Vec<PlaylistLite> = app.playlists.iter().map(PlaylistLite::from).collect();

    // 诊断日志：记录歌单保存信息
    tracing::info!(
        "🎵 [StateSave] 保存歌单: count={}, playlists_selected={}",
        playlists.len(),
        app.playlists_selected
    );
    for (i, p) in playlists.iter().enumerate() {
        tracing::info!(
            "🎵 [StateSave]   歌单[{}]: id={}, name={}, track_count={}, special_type={}",
            i,
            p.id,
            p.name,
            p.track_count,
            p.special_type
        );
    }

    let player = PlayerState {
        version: CURRENT_VERSION,
        play_song_id: app.play_song_id,
        progress: PlaybackProgress {
            started_at_epoch_ms,
            total_ms: app.play_total_ms,
            paused: app.paused,
            paused_at_epoch_ms,
            paused_accum_ms: app.play_paused_accum_ms,
        },
        play_queue,
        volume: app.volume,
        play_br: app.play_br,
        crossfade_ms: app.crossfade_ms,
    };

    // 保存预加载的歌单数据
    let playlist_preloads = app.playlist_preloads.clone();

    // 诊断日志：记录预加载歌单保存信息
    tracing::info!(
        "🎵 [StateSave] 保存 playlist_preloads: count={}",
        playlist_preloads.len()
    );
    for (id, preload) in &playlist_preloads {
        tracing::info!(
            "🎵 [StateSave]   预加载歌单[{}]: status={:?}, songs={}",
            id,
            preload.status,
            preload.songs.len()
        );
    }

    AppStateSnapshot {
        version: CURRENT_VERSION,
        player,
        playlists,
        playlists_selected: app.playlists_selected,
        playlist_preloads,
        saved_at_epoch_ms: now,
    }
}

/// 从持久化格式恢复到 App
pub fn apply_snapshot_to_app(
    snapshot: &AppStateSnapshot,
    app: &mut App,
) -> Result<(), PlayerStateError> {
    // 检查版本兼容性（支持版本 1、2 和 3）
    if snapshot.version > CURRENT_VERSION {
        return Err(PlayerStateError::IncompatibleVersion {
            expected: CURRENT_VERSION,
            found: snapshot.version,
        });
    }

    // 版本 1 没有 special_type 字段，恢复时使用默认值
    let use_default_special_type = snapshot.version < 2;

    // 版本 3 恢复 playlist_preloads，版本 1-2 使用空 HashMap
    if snapshot.version >= 3 {
        app.playlist_preloads = snapshot.playlist_preloads.clone();
        tracing::info!(
            "🎵 [StateRestore] 恢复 playlist_preloads: count={}",
            app.playlist_preloads.len()
        );
        // 详细日志：记录每个预加载歌单的状态
        for (id, preload) in &app.playlist_preloads {
            tracing::info!(
                "🎵 [StateRestore]   预加载歌单[{}]: status={:?}, songs={}",
                id,
                preload.status,
                preload.songs.len()
            );
        }
    } else {
        app.playlist_preloads = HashMap::new();
        tracing::info!("🎵 [StateRestore] 版本 < 3, playlist_preloads 初始化为空");
    }

    let now = chrono::Utc::now().timestamp_millis();
    let time_since_save = now - snapshot.saved_at_epoch_ms;

    // 恢复播放进度
    if let Some(started_at) = snapshot.player.progress.started_at_epoch_ms {
        // 计算新的 started_at，使得播放位置不变
        // 新的 started_at = 当前时间 - (保存时的播放时长 + 距离保存的时间)
        let save_time_elapsed = now - started_at;
        let new_elapsed_ms = (save_time_elapsed + time_since_save) as u64;

        // 如果暂停，使用暂停时的播放位置
        let final_elapsed_ms = if snapshot.player.progress.paused {
            new_elapsed_ms.saturating_sub(snapshot.player.progress.paused_accum_ms)
        } else {
            new_elapsed_ms
        };

        tracing::info!(
            "🎵 [StateRestore] 恢复播放状态: saved_elapsed_ms={}ms, now={}, time_since_save={}ms, final_elapsed_ms={}ms, paused={}, paused_accum_ms={}ms",
            save_time_elapsed,
            now,
            time_since_save,
            final_elapsed_ms,
            snapshot.player.progress.paused,
            snapshot.player.progress.paused_accum_ms,
        );

        app.play_started_at = Some(Instant::now() - Duration::from_millis(final_elapsed_ms));
    } else {
        tracing::info!("🎵 [StateRestore] 没有保存的播放进度，play_started_at 设置为 None");
        app.play_started_at = None;
    }

    app.play_total_ms = snapshot.player.progress.total_ms;
    app.paused = true; // 默认恢复为暂停
    app.play_paused_accum_ms = snapshot.player.progress.paused_accum_ms;
    app.play_paused_at = if snapshot.player.progress.paused {
        Some(Instant::now())
    } else {
        None
    };

    // 恢复播放器状态
    app.play_song_id = snapshot.player.play_song_id;
    app.volume = snapshot.player.volume;
    app.play_br = snapshot.player.play_br;
    app.crossfade_ms = snapshot.player.crossfade_ms;
    app.play_mode = play_mode_from_string(&snapshot.player.play_queue.mode);

    // 恢复播放队列
    let songs: Vec<Song> = snapshot
        .player
        .play_queue
        .songs
        .iter()
        .map(|lite| Song {
            id: lite.id,
            name: lite.name.clone(),
            artists: lite.artists.clone(),
        })
        .collect();

    app.play_queue = PlayQueue::new(app.play_mode);
    app.play_queue.set_songs(songs, None);
    if let Some(cursor) = snapshot.player.play_queue.cursor {
        app.play_queue.set_cursor_pos(cursor);
    }

    // 恢复歌单（只恢复基本信息，不恢复歌曲详情）
    app.playlists = snapshot
        .playlists
        .iter()
        .map(|lite| Playlist {
            id: lite.id,
            name: lite.name.clone(),
            track_count: lite.track_count,
            special_type: if use_default_special_type {
                0 // 版本 1 兼容：旧数据没有 special_type
            } else {
                lite.special_type
            },
        })
        .collect();

    // 诊断日志：记录恢复的歌单信息
    tracing::info!(
        "🎵 [StateRestore] 恢复歌单: count={}, playlists_selected={}",
        app.playlists.len(),
        snapshot.playlists_selected
    );
    for (i, p) in app.playlists.iter().enumerate() {
        tracing::info!(
            "🎵 [StateRestore]   歌单[{}]: id={}, name={}, track_count={}, special_type={}",
            i,
            p.id,
            p.name,
            p.track_count,
            p.special_type
        );
    }

    // 边界检查警告
    if snapshot.playlists_selected >= app.playlists.len() && !app.playlists.is_empty() {
        tracing::warn!(
            "🎵 [StateRestore] playlists_selected 越界: {} >= {}, 将被截断",
            snapshot.playlists_selected,
            app.playlists.len()
        );
    }

    // 边界检查：确保 playlists_selected 不越界
    app.playlists_selected = if !app.playlists.is_empty() {
        std::cmp::min(snapshot.playlists_selected, app.playlists.len() - 1)
    } else {
        0
    };

    Ok(())
}

/// 加载播放器状态
pub fn load_player_state(data_dir: &Path) -> Result<AppStateSnapshot, PlayerStateError> {
    let path = state_path(data_dir);
    let bytes = fs::read(&path).map_err(PlayerStateError::Io)?;
    let snapshot: AppStateSnapshot =
        serde_json::from_slice(&bytes).map_err(PlayerStateError::Serde)?;

    // 检查版本兼容性（支持版本 1 和 2）
    if snapshot.version > CURRENT_VERSION {
        return Err(PlayerStateError::IncompatibleVersion {
            expected: CURRENT_VERSION,
            found: snapshot.version,
        });
    }

    Ok(snapshot)
}

/// 保存播放器状态
pub fn save_player_state(data_dir: &Path, app: &App) -> Result<(), PlayerStateError> {
    fs::create_dir_all(data_dir).map_err(PlayerStateError::Io)?;

    let path = state_path(data_dir);
    let tmp_path = path.with_extension("json.tmp");

    let snapshot = app_to_snapshot(app);

    // 计算播放进度用于日志
    let elapsed_ms = playback_elapsed_ms(app);
    let started_at_epoch_ms = snapshot.player.progress.started_at_epoch_ms;
    let now = chrono::Utc::now().timestamp_millis();

    tracing::info!(
        "🎵 [StateSave] 保存播放状态: elapsed_ms={}s, started_at_epoch_ms={:?}, paused={}, paused_accum_ms={}ms",
        elapsed_ms / 1000,
        started_at_epoch_ms.map(|t| format!("{} (前{}ms)", t, now - t)),
        app.paused,
        app.play_paused_accum_ms,
    );

    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(PlayerStateError::Serde)?;

    fs::write(&tmp_path, bytes).map_err(PlayerStateError::Io)?;

    // 原子性写入
    if let Err(e) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(PlayerStateError::Io(e));
    }

    Ok(())
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

fn play_mode_to_string(m: PlayMode) -> String {
    match m {
        PlayMode::Sequential => "Sequential",
        PlayMode::ListLoop => "ListLoop",
        PlayMode::SingleLoop => "SingleLoop",
        PlayMode::Shuffle => "Shuffle",
    }
    .to_owned()
}

fn play_mode_from_string(s: &str) -> PlayMode {
    match s {
        "Sequential" => PlayMode::Sequential,
        "SingleLoop" => PlayMode::SingleLoop,
        "Shuffle" => PlayMode::Shuffle,
        _ => PlayMode::ListLoop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PlayQueue;

    #[test]
    fn test_song_lite_from_song() {
        let song = Song {
            id: 123,
            name: "Test Song".to_string(),
            artists: "Test Artist".to_string(),
        };

        let lite = SongLite::from(&song);
        assert_eq!(lite.id, 123);
        assert_eq!(lite.name, "Test Song");
        assert_eq!(lite.artists, "Test Artist");
    }

    #[test]
    fn test_play_mode_conversion() {
        assert_eq!(play_mode_to_string(PlayMode::Sequential), "Sequential");
        assert_eq!(play_mode_to_string(PlayMode::ListLoop), "ListLoop");
        assert_eq!(play_mode_to_string(PlayMode::SingleLoop), "SingleLoop");
        assert_eq!(play_mode_to_string(PlayMode::Shuffle), "Shuffle");

        assert_eq!(play_mode_from_string("Sequential"), PlayMode::Sequential);
        assert_eq!(play_mode_from_string("ListLoop"), PlayMode::ListLoop);
        assert_eq!(play_mode_from_string("SingleLoop"), PlayMode::SingleLoop);
        assert_eq!(play_mode_from_string("Shuffle"), PlayMode::Shuffle);
        assert_eq!(
            play_mode_from_string("Invalid"),
            PlayMode::ListLoop // 默认值
        );
    }

    #[test]
    fn test_playlist_lite_from_playlist() {
        let playlist = Playlist {
            id: 456,
            name: "Test Playlist".to_string(),
            track_count: 100,
            special_type: 0,
        };

        let lite = PlaylistLite::from(&playlist);
        assert_eq!(lite.id, 456);
        assert_eq!(lite.name, "Test Playlist");
        assert_eq!(lite.track_count, 100);
        assert_eq!(lite.special_type, 0);
    }

    #[test]
    fn test_playback_elapsed_ms_no_start() {
        let app = App::default();
        let elapsed = playback_elapsed_ms(&app);
        assert_eq!(elapsed, 0);
    }

    #[test]
    fn test_player_state_error_display() {
        let err = PlayerStateError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(err.to_string().contains("IO 错误"));

        let err = PlayerStateError::IncompatibleVersion {
            expected: 1,
            found: 2,
        };
        assert!(err.to_string().contains("版本不兼容"));
    }

    #[test]
    fn test_apply_snapshot_incompatible_version() {
        let snapshot = AppStateSnapshot {
            version: 99, // 不兼容的版本
            player: PlayerState {
                version: 99,
                play_song_id: None,
                progress: PlaybackProgress {
                    started_at_epoch_ms: None,
                    total_ms: None,
                    paused: true,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 0.5,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: HashMap::new(),
            saved_at_epoch_ms: 0,
        };

        let mut app = App::default();
        let result = apply_snapshot_to_app(&snapshot, &mut app);
        assert!(result.is_err());
        match result {
            Err(PlayerStateError::IncompatibleVersion { expected, found }) => {
                assert_eq!(expected, 3);
                assert_eq!(found, 99);
            }
            _ => panic!("Expected IncompatibleVersion error"),
        }
    }

    #[test]
    fn test_apply_snapshot_basic() {
        let snapshot = AppStateSnapshot {
            version: 1,
            player: PlayerState {
                version: 1,
                play_song_id: Some(123),
                progress: PlaybackProgress {
                    started_at_epoch_ms: None,
                    total_ms: Some(180000),
                    paused: true,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 5000,
                },
                play_queue: PlayQueueState {
                    songs: vec![SongLite {
                        id: 123,
                        name: "Test Song".to_string(),
                        artists: "Test Artist".to_string(),
                    }],
                    order: vec![0],
                    cursor: Some(0),
                    mode: "ListLoop".to_string(),
                },
                volume: 0.7,
                play_br: 320000,
                crossfade_ms: 500,
            },
            playlists: vec![PlaylistLite {
                id: 1,
                name: "My Playlist".to_string(),
                track_count: 50,
                special_type: 0,
            }],
            playlists_selected: 0,
            playlist_preloads: HashMap::new(),
            saved_at_epoch_ms: chrono::Utc::now().timestamp_millis(),
        };

        let mut app = App::default();
        let result = apply_snapshot_to_app(&snapshot, &mut app);
        assert!(result.is_ok());

        // 验证恢复的状态
        assert_eq!(app.play_song_id, Some(123));
        assert_eq!(app.volume, 0.7);
        assert_eq!(app.play_br, 320000);
        assert_eq!(app.crossfade_ms, 500);
        assert_eq!(app.play_total_ms, Some(180000));
        assert!(app.paused); // 默认恢复为暂停
        assert_eq!(app.play_paused_accum_ms, 5000);
        assert_eq!(app.playlists.len(), 1);
        assert_eq!(app.playlists[0].id, 1);
        assert_eq!(app.play_mode, PlayMode::ListLoop);
    }

    #[test]
    fn test_playqueue_set_cursor_pos() {
        let mut queue = PlayQueue::new(PlayMode::Sequential);

        // 空队列时设置 cursor
        queue.set_cursor_pos(0);
        assert_eq!(queue.cursor_pos(), None);

        // 添加歌曲
        let songs = vec![
            Song {
                id: 1,
                name: "Song 1".to_string(),
                artists: "Artist 1".to_string(),
            },
            Song {
                id: 2,
                name: "Song 2".to_string(),
                artists: "Artist 2".to_string(),
            },
        ];
        queue.set_songs(songs, None);

        // 有效位置
        queue.set_cursor_pos(1);
        assert_eq!(queue.cursor_pos(), Some(1));

        // 超出范围
        queue.set_cursor_pos(10);
        assert_eq!(queue.cursor_pos(), None);
    }

    #[test]
    fn test_playlist_preloads_serialization() {
        use crate::app::{PlaylistPreload, PreloadStatus};

        // 创建包含预加载歌单的快照
        let preload = PlaylistPreload {
            status: PreloadStatus::Completed,
            songs: vec![Song {
                id: 101,
                name: "Preloaded Song".to_string(),
                artists: "Test Artist".to_string(),
            }],
        };

        // 验证 PlaylistPreload 可以序列化和反序列化
        let serialized = serde_json::to_string(&preload).expect("序列化失败");
        let deserialized: PlaylistPreload =
            serde_json::from_str(&serialized).expect("反序列化失败");

        match deserialized.status {
            PreloadStatus::Completed => {}
            _ => panic!("期望 Completed 状态"),
        }
        assert_eq!(deserialized.songs.len(), 1);
        assert_eq!(deserialized.songs[0].id, 101);
    }

    #[test]
    fn test_app_state_snapshot_with_playlist_preloads() {
        use crate::app::{PlaylistPreload, PreloadStatus};

        // 创建版本 3 快照，包含 playlist_preloads
        let snapshot_with_preloads = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: None,
                progress: PlaybackProgress {
                    started_at_epoch_ms: None,
                    total_ms: None,
                    paused: true,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 0.5,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![PlaylistLite {
                id: 1,
                name: "Test Playlist".to_string(),
                track_count: 10,
                special_type: 0,
            }],
            playlists_selected: 0,
            playlist_preloads: vec![(
                1,
                PlaylistPreload {
                    status: PreloadStatus::Completed,
                    songs: vec![Song {
                        id: 201,
                        name: "Cached Song".to_string(),
                        artists: "Cached Artist".to_string(),
                    }],
                },
            )]
            .into_iter()
            .collect(),
            saved_at_epoch_ms: chrono::Utc::now().timestamp_millis(),
        };

        // 验证快照可以序列化和反序列化
        let serialized = serde_json::to_string(&snapshot_with_preloads).expect("序列化失败");
        let deserialized: AppStateSnapshot =
            serde_json::from_str(&serialized).expect("反序列化失败");

        assert_eq!(deserialized.version, 3);
        assert_eq!(deserialized.playlist_preloads.len(), 1);
        assert!(deserialized.playlist_preloads.contains_key(&1));

        let preload = &deserialized.playlist_preloads[&1];
        assert_eq!(preload.songs.len(), 1);
        assert_eq!(preload.songs[0].id, 201);
    }

    #[test]
    fn test_apply_snapshot_restores_playlist_preloads_v3() {
        use crate::app::{PlaylistPreload, PreloadStatus};

        // 创建版本 3 快照
        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: None,
                progress: PlaybackProgress {
                    started_at_epoch_ms: None,
                    total_ms: None,
                    paused: true,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 0.5,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![PlaylistLite {
                id: 100,
                name: "My Playlist".to_string(),
                track_count: 50,
                special_type: 5,
            }],
            playlists_selected: 0,
            playlist_preloads: vec![(
                100,
                PlaylistPreload {
                    status: PreloadStatus::Completed,
                    songs: vec![
                        Song {
                            id: 301,
                            name: "Song A".to_string(),
                            artists: "Artist A".to_string(),
                        },
                        Song {
                            id: 302,
                            name: "Song B".to_string(),
                            artists: "Artist B".to_string(),
                        },
                    ],
                },
            )]
            .into_iter()
            .collect(),
            saved_at_epoch_ms: chrono::Utc::now().timestamp_millis(),
        };

        let mut app = App::default();
        let result = apply_snapshot_to_app(&snapshot, &mut app);

        assert!(result.is_ok(), "apply_snapshot_to_app 应该成功");

        // 验证 playlist_preloads 被恢复
        assert_eq!(app.playlist_preloads.len(), 1, "应该恢复 1 个预加载歌单");
        assert!(
            app.playlist_preloads.contains_key(&100),
            "应该包含歌单 ID 100 的预加载数据"
        );

        let preload = &app.playlist_preloads[&100];
        assert_eq!(preload.songs.len(), 2, "应该有 2 首歌曲");
        assert_eq!(preload.songs[0].id, 301);
        assert_eq!(preload.songs[1].id, 302);

        match &preload.status {
            PreloadStatus::Completed => {}
            _ => panic!("期望 Completed 状态"),
        }
    }
}
