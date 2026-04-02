use crate::app::state::{App, PlayMode};
use crate::app::{PlayQueue, PlaylistPreload};
use crate::domain::model::{Playlist, Song};
use crate::error::PlayerStateError;
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
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

impl From<&Song> for SongLite {
    fn from(song: &Song) -> Self {
        Self {
            id: song.id,
            name: song.name.clone(),
            artists: song.artists.clone(),
            duration_ms: song.duration_ms,
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

/// 将 App 转换为持久化格式
fn app_to_snapshot(app: &App) -> AppStateSnapshot {
    let now = chrono::Utc::now().timestamp_millis();

    // 计算播放进度
    let elapsed_ms = app.playback_elapsed_ms();

    // 反推 started_at 时间戳：saved_at - elapsed = started_at
    let started_at_epoch_ms = if elapsed_ms > 0 {
        let elapsed_ms_i64 = i64::try_from(elapsed_ms).unwrap_or(i64::MAX);
        Some(now.saturating_sub(elapsed_ms_i64))
    } else {
        None
    };

    // 计算暂停累积时间的时间戳
    let paused_at_epoch_ms = if app.paused {
        if let Some(paused_at) = app.play_paused_at {
            // paused_at 是 Instant，需要转换为时间戳
            // paused_at_epoch_ms = now - (now - paused_at)
            let paused_elapsed_ms_i64 =
                i64::try_from(paused_at.elapsed().as_millis()).unwrap_or(i64::MAX);
            Some(now.saturating_sub(paused_elapsed_ms_i64))
        } else {
            Some(now)
        }
    } else {
        None
    };

    // 转换播放队列
    let play_queue = PlayQueueState {
        songs: app.play_queue.songs().iter().map(SongLite::from).collect(),
        order: app.play_queue.order().to_vec(),
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

    let now_epoch_ms = chrono::Utc::now().timestamp_millis();
    let restore_now = Instant::now();
    let time_since_save_ms = now_epoch_ms
        .saturating_sub(snapshot.saved_at_epoch_ms)
        .max(0);
    if snapshot.saved_at_epoch_ms > now_epoch_ms {
        tracing::warn!(
            saved_at_epoch_ms = snapshot.saved_at_epoch_ms,
            now_epoch_ms,
            "🎵 [StateRestore] saved_at 在未来（可能是系统时间回拨/状态文件异常），将忽略 time_since_save"
        );
    }

    // 恢复播放进度（以“播放位置 ms”作为主语义）
    //
    // Snapshot 中的 `started_at_epoch_ms` 是用 `saved_at_epoch_ms - position_ms` 反推的“虚拟 started_at”，
    // position_ms 需要额外结合 `paused_accum_ms` 才能恢复到 App 的 `Instant` 模型。
    //
    // 额外：为支持“异常退出/崩溃”场景的近似恢复，如果保存时未暂停，则最多补偿一个 autosave 周期（30s）。
    const MAX_ADVANCE_MS: i64 = 30_000;

    let paused_accum_ms_u64 = snapshot.player.progress.paused_accum_ms;
    let base_pos_ms = snapshot
        .player
        .progress
        .started_at_epoch_ms
        .map(|started_epoch_ms| snapshot.saved_at_epoch_ms.saturating_sub(started_epoch_ms))
        .unwrap_or(0)
        .max(0);

    let advance_ms = if snapshot.player.progress.paused {
        0
    } else {
        time_since_save_ms.min(MAX_ADVANCE_MS)
    };

    let mut pos_ms_i64 = base_pos_ms.saturating_add(advance_ms).max(0);
    if let Some(total_ms) = snapshot.player.progress.total_ms {
        let total_ms_i64 = i64::try_from(total_ms).unwrap_or(i64::MAX);
        pos_ms_i64 = pos_ms_i64.min(total_ms_i64);
    }

    tracing::info!(
        "🎵 [StateRestore] 恢复播放进度: base_pos_ms={}ms, advance_ms={}ms, final_pos_ms={}ms, saved_paused={}, paused_accum_ms={}ms",
        base_pos_ms,
        advance_ms,
        pos_ms_i64,
        snapshot.player.progress.paused,
        paused_accum_ms_u64,
    );

    let pos_ms_u64 = u64::try_from(pos_ms_i64).unwrap_or(u64::MAX);
    let total_offset_ms_u64 = pos_ms_u64.saturating_add(paused_accum_ms_u64);
    let started_at = restore_now
        .checked_sub(Duration::from_millis(total_offset_ms_u64))
        .or_else(|| {
            tracing::warn!(
                total_offset_ms_u64,
                "🎵 [StateRestore] 播放进度过大导致 Instant::checked_sub 失败，将丢弃 play_started_at"
            );
            None
        });

    app.play_started_at = started_at;

    app.play_total_ms = snapshot.player.progress.total_ms;
    app.paused = true; // 默认恢复为暂停
    app.play_paused_accum_ms = paused_accum_ms_u64;
    // 恢复时总是“暂停”以避免自动播放；如果有 started_at，则冻结 paused_at 以避免进度继续走。
    app.play_paused_at = app.play_started_at.map(|_| restore_now);

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
            duration_ms: lite.duration_ms,
        })
        .collect();

    app.play_queue = PlayQueue::new(app.play_mode);
    if !app.play_queue.restore(
        songs,
        snapshot.player.play_queue.order.clone(),
        snapshot.player.play_queue.cursor,
    ) {
        tracing::warn!(
            order_len = snapshot.player.play_queue.order.len(),
            songs_len = snapshot.player.play_queue.songs.len(),
            "🎵 [StateRestore] 保存的播放队列顺序无效，已回退到自然顺序"
        );
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
#[allow(dead_code)]
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

/// 异步加载播放器状态（避免在 async 运行时中执行阻塞 IO）
pub async fn load_player_state_async(
    data_dir: &Path,
) -> Result<AppStateSnapshot, PlayerStateError> {
    let path = state_path(data_dir);
    let bytes = tokio::fs::read(&path).await.map_err(PlayerStateError::Io)?;
    let snapshot: AppStateSnapshot =
        serde_json::from_slice(&bytes).map_err(PlayerStateError::Serde)?;

    if snapshot.version > CURRENT_VERSION {
        return Err(PlayerStateError::IncompatibleVersion {
            expected: CURRENT_VERSION,
            found: snapshot.version,
        });
    }

    Ok(snapshot)
}

/// 保存播放器状态
#[allow(dead_code)]
pub fn save_player_state(data_dir: &Path, app: &App) -> Result<(), PlayerStateError> {
    fs::create_dir_all(data_dir).map_err(PlayerStateError::Io)?;

    let path = state_path(data_dir);
    let tmp_path = path.with_extension("json.tmp");

    let snapshot = app_to_snapshot(app);

    // 计算播放进度用于日志
    let elapsed_ms = app.playback_elapsed_ms();
    let started_at_epoch_ms = snapshot.player.progress.started_at_epoch_ms;
    let now = chrono::Utc::now().timestamp_millis();

    tracing::info!(
        "🎵 [StateSave] 保存播放状态: elapsed_ms={}s, started_at_epoch_ms={:?}, paused={}, paused_accum_ms={}ms",
        elapsed_ms / 1000,
        started_at_epoch_ms.map(|t| format!("{} (前{}ms)", t, now.saturating_sub(t))),
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

/// 异步保存播放器状态（避免在 async 运行时中执行阻塞 IO）
///
/// 为避免将 `&App` 跨任务借用，本函数接收 `App` 的所有权（调用方可传 `app.clone()`）。
pub async fn save_player_state_async(data_dir: &Path, app: App) -> Result<(), PlayerStateError> {
    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(PlayerStateError::Io)?;

    let path = state_path(data_dir);
    let tmp_path = path.with_extension("json.tmp");

    let snapshot = app_to_snapshot(&app);
    let base_pos_ms = snapshot
        .player
        .progress
        .started_at_epoch_ms
        .map(|t| snapshot.saved_at_epoch_ms.saturating_sub(t))
        .unwrap_or(0)
        .max(0);
    tracing::trace!(
        path = %path.display(),
        saved_at_epoch_ms = snapshot.saved_at_epoch_ms,
        started_at_epoch_ms = snapshot.player.progress.started_at_epoch_ms,
        base_pos_ms,
        paused = snapshot.player.progress.paused,
        paused_at_epoch_ms = snapshot.player.progress.paused_at_epoch_ms,
        paused_accum_ms = snapshot.player.progress.paused_accum_ms,
        total_ms = ?snapshot.player.progress.total_ms,
        play_song_id = ?snapshot.player.play_song_id,
        "🎵 [StateSaveDbg] snapshot"
    );
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(PlayerStateError::Serde)?;

    tokio::fs::write(&tmp_path, bytes)
        .await
        .map_err(PlayerStateError::Io)?;

    // 尽量保持原子写入语义（Windows 上 rename 不能覆盖已存在目标）
    match tokio::fs::rename(&tmp_path, &path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // 如果目标已存在，尝试删除后再 rename（Windows 上常见）
            tracing::debug!(err = %e, "player_state rename failed, retrying with remove_file");
            let _ = tokio::fs::remove_file(&path).await;
            match tokio::fs::rename(&tmp_path, &path).await {
                Ok(()) => Ok(()),
                Err(e2) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    Err(PlayerStateError::Io(e2))
                }
            }
        }
    }
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

    fn song(id: i64, name: &str, artists: &str) -> Song {
        Song {
            id,
            name: name.to_string(),
            artists: artists.to_string(),
            duration_ms: None,
        }
    }

    fn app_playback_elapsed_ms(app: &App) -> u64 {
        app.playback_elapsed_ms()
    }

    #[test]
    fn test_song_lite_from_song() {
        let song = song(123, "Test Song", "Test Artist");

        let lite = SongLite::from(&song);
        assert_eq!(lite.id, 123);
        assert_eq!(lite.name, "Test Song");
        assert_eq!(lite.artists, "Test Artist");
        assert_eq!(lite.duration_ms, None);
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
    fn test_apply_snapshot_handles_extreme_timestamps_without_panic() {
        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: Some(1),
                progress: PlaybackProgress {
                    started_at_epoch_ms: Some(i64::MIN),
                    total_ms: Some(1_000),
                    paused: false,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 1.0,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: std::collections::HashMap::new(),
            saved_at_epoch_ms: i64::MIN,
        };

        let mut app = App::default();
        let result = apply_snapshot_to_app(&snapshot, &mut app);
        assert!(result.is_ok());
        assert!(app.play_started_at.is_some());
    }

    #[test]
    fn test_apply_snapshot_handles_future_timestamps() {
        let now = chrono::Utc::now().timestamp_millis();
        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: Some(1),
                progress: PlaybackProgress {
                    started_at_epoch_ms: Some(now + 10_000),
                    total_ms: Some(180_000),
                    paused: false,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 1.0,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: std::collections::HashMap::new(),
            saved_at_epoch_ms: now + 5_000,
        };

        let mut app = App::default();
        let result = apply_snapshot_to_app(&snapshot, &mut app);
        assert!(result.is_ok());
        assert!(app.play_started_at.is_some());
    }

    #[test]
    fn test_apply_snapshot_restores_position_with_capped_advance() {
        let now = chrono::Utc::now().timestamp_millis();
        let saved_at = now - 120_000; // 2 minutes ago

        let base_pos_ms: i64 = 10_000;
        let started_at_epoch_ms = saved_at - base_pos_ms; // virtual started_at: saved_at - position

        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: Some(1),
                progress: PlaybackProgress {
                    started_at_epoch_ms: Some(started_at_epoch_ms),
                    total_ms: Some(180_000),
                    paused: false,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 5_000,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 1.0,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: std::collections::HashMap::new(),
            saved_at_epoch_ms: saved_at,
        };

        let mut app = App::default();
        apply_snapshot_to_app(&snapshot, &mut app).unwrap();

        // Restore always pauses, so `play_paused_at` must be set to freeze the elapsed time.
        assert!(app.paused);
        assert!(app.play_paused_at.is_some());

        // Since we cap advance to one autosave cycle (30s): 10s + 30s = 40s.
        assert_eq!(app_playback_elapsed_ms(&app), 40_000);
    }

    #[test]
    fn test_apply_snapshot_restores_position_paused_does_not_advance() {
        let now = chrono::Utc::now().timestamp_millis();
        let saved_at = now - 120_000; // 2 minutes ago

        let base_pos_ms: i64 = 10_000;
        let started_at_epoch_ms = saved_at - base_pos_ms;

        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: Some(1),
                progress: PlaybackProgress {
                    started_at_epoch_ms: Some(started_at_epoch_ms),
                    total_ms: Some(180_000),
                    paused: true,
                    paused_at_epoch_ms: Some(saved_at),
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![],
                    order: vec![],
                    cursor: None,
                    mode: "ListLoop".to_string(),
                },
                volume: 1.0,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: std::collections::HashMap::new(),
            saved_at_epoch_ms: saved_at,
        };

        let mut app = App::default();
        apply_snapshot_to_app(&snapshot, &mut app).unwrap();

        // paused-at-save should never advance.
        assert_eq!(app_playback_elapsed_ms(&app), 10_000);
    }

    #[test]
    fn test_playback_elapsed_ms_no_start() {
        let app = App::default();
        let elapsed = app_playback_elapsed_ms(&app);
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
                        duration_ms: Some(180000),
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
    fn test_apply_snapshot_restores_queue_order_and_none_cursor() {
        let snapshot = AppStateSnapshot {
            version: 3,
            player: PlayerState {
                version: 3,
                play_song_id: Some(3),
                progress: PlaybackProgress {
                    started_at_epoch_ms: None,
                    total_ms: Some(180_000),
                    paused: true,
                    paused_at_epoch_ms: None,
                    paused_accum_ms: 0,
                },
                play_queue: PlayQueueState {
                    songs: vec![
                        SongLite {
                            id: 1,
                            name: "Song 1".to_string(),
                            artists: "Artist".to_string(),
                            duration_ms: None,
                        },
                        SongLite {
                            id: 2,
                            name: "Song 2".to_string(),
                            artists: "Artist".to_string(),
                            duration_ms: None,
                        },
                        SongLite {
                            id: 3,
                            name: "Song 3".to_string(),
                            artists: "Artist".to_string(),
                            duration_ms: None,
                        },
                    ],
                    order: vec![2, 0, 1],
                    cursor: None,
                    mode: "Shuffle".to_string(),
                },
                volume: 0.7,
                play_br: 320000,
                crossfade_ms: 300,
            },
            playlists: vec![],
            playlists_selected: 0,
            playlist_preloads: HashMap::new(),
            saved_at_epoch_ms: chrono::Utc::now().timestamp_millis(),
        };

        let mut app = App::default();
        apply_snapshot_to_app(&snapshot, &mut app).unwrap();

        assert_eq!(app.play_mode, PlayMode::Shuffle);
        assert_eq!(app.play_queue.order(), &[2, 0, 1]);
        assert_eq!(app.play_queue.cursor_pos(), None);
        assert!(app.play_queue.current().is_none());
        let ordered_ids: Vec<_> = app
            .play_queue
            .ordered_songs()
            .iter()
            .map(|song| song.id)
            .collect();
        assert_eq!(ordered_ids, vec![3, 1, 2]);
    }

    #[test]
    fn test_app_to_snapshot_preserves_order_with_duplicate_song_ids() {
        let mut app = App {
            play_mode: PlayMode::Shuffle,
            ..App::default()
        };
        app.play_queue = PlayQueue::new(PlayMode::Shuffle);
        let restored = app.play_queue.restore(
            vec![
                song(1, "Song A", "Artist"),
                song(1, "Song A duplicate", "Artist"),
                song(2, "Song B", "Artist"),
            ],
            vec![1, 0, 2],
            Some(1),
        );
        assert!(restored);

        let snapshot = app_to_snapshot(&app);
        assert_eq!(snapshot.player.play_queue.order, vec![1, 0, 2]);
        assert_eq!(snapshot.player.play_queue.cursor, Some(1));
        let song_ids: Vec<_> = snapshot
            .player
            .play_queue
            .songs
            .iter()
            .map(|song| song.id)
            .collect();
        assert_eq!(song_ids, vec![1, 1, 2]);
    }

    #[test]
    fn test_playlist_preloads_serialization() {
        use crate::app::{PlaylistPreload, PreloadStatus};

        // 创建包含预加载歌单的快照
        let preload = PlaylistPreload {
            status: PreloadStatus::Completed,
            songs: vec![song(101, "Preloaded Song", "Test Artist")],
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
                    songs: vec![song(201, "Cached Song", "Cached Artist")],
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
                        song(301, "Song A", "Artist A"),
                        song(302, "Song B", "Artist B"),
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
