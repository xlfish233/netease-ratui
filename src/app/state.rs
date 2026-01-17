use std::collections::HashMap;
use std::time::Instant;

use super::PlayQueue;
use crate::domain::model::LyricLine;

pub use crate::domain::model::{Playlist, Song};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Login,
    Playlists,
    Search,
    Lyrics,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiFocus {
    HeaderSearch,
    BodyLeft,
    BodyCenter,
    BodyRight,
}

/// 标签页配置：统一管理标题与对应的 View
#[derive(Debug, Clone, Copy)]
pub struct TabConfig {
    pub title: &'static str,
    pub view: View,
}

/// 获取当前登录状态下的标签页配置
pub fn tab_configs(logged_in: bool) -> &'static [TabConfig] {
    if logged_in {
        &[
            TabConfig {
                title: "歌单",
                view: View::Playlists,
            },
            TabConfig {
                title: "搜索",
                view: View::Search,
            },
            TabConfig {
                title: "歌词",
                view: View::Lyrics,
            },
            TabConfig {
                title: "设置",
                view: View::Settings,
            },
        ]
    } else {
        &[
            TabConfig {
                title: "登录",
                view: View::Login,
            },
            TabConfig {
                title: "搜索",
                view: View::Search,
            },
            TabConfig {
                title: "歌词",
                view: View::Lyrics,
            },
            TabConfig {
                title: "设置",
                view: View::Settings,
            },
        ]
    }
}

/// 根据 View 查找其在标签页列表中的索引
pub fn tab_index_for_view(view: View, logged_in: bool) -> Option<usize> {
    tab_configs(logged_in).iter().position(|c| c.view == view)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistMode {
    List,
    Tracks,
}

#[derive(Debug, Clone)]
pub struct PlaylistPreload {
    pub status: PreloadStatus,
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone)]
pub enum PreloadStatus {
    #[allow(dead_code)]
    NotStarted,
    Loading {
        loaded: usize,
        total: usize,
    },
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Sequential,
    ListLoop,
    SingleLoop,
    Shuffle,
}

#[derive(Debug, Clone)]
pub struct App {
    pub view: View,
    pub ui_focus: UiFocus,
    pub help_visible: bool,

    pub login_qr_url: Option<String>,
    pub login_qr_ascii: Option<String>,
    pub login_unikey: Option<String>,
    pub login_status: String,
    pub logged_in: bool,
    pub login_cookie_input: String,
    pub login_cookie_input_visible: bool,

    pub search_input: String,
    pub search_results: Vec<Song>,
    pub search_selected: usize,
    pub search_status: String,

    pub now_playing: Option<String>,
    pub play_status: String,
    pub paused: bool,
    pub play_started_at: Option<Instant>,
    pub play_total_ms: Option<u64>,
    pub play_paused_at: Option<Instant>,
    pub play_paused_accum_ms: u64,
    pub pending_seek_ms: Option<u64>,
    pub play_id: Option<u64>,
    pub play_queue: PlayQueue,
    pub play_mode: PlayMode,
    pub volume: f32,
    pub play_song_id: Option<i64>,
    pub play_error_count: u32,
    pub play_br: i64,
    pub crossfade_ms: u64,

    pub account_uid: Option<i64>,
    pub account_nickname: Option<String>,
    pub playlists: Vec<Playlist>,
    pub playlists_selected: usize,
    pub playlist_mode: PlaylistMode,
    pub playlist_tracks: Vec<Song>,
    pub playlist_tracks_selected: usize,
    pub playlists_status: String,

    pub playlist_preloads: HashMap<i64, PlaylistPreload>,
    pub preload_summary: String,

    pub lyrics_song_id: Option<i64>,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_status: String,
    pub lyrics_follow: bool,
    pub lyrics_selected: usize,
    pub lyrics_offset_ms: i64,

    pub settings_selected: usize,
    pub settings_status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            view: View::Login,
            ui_focus: UiFocus::BodyCenter,
            help_visible: false,
            login_qr_url: None,
            login_qr_ascii: None,
            login_unikey: None,
            login_status: "按 l 生成二维码；q 退出；Ctrl+Tab 切换页面".to_owned(),
            logged_in: false,
            login_cookie_input: String::new(),
            login_cookie_input_visible: false,
            search_input: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_status: "输入关键词，回车搜索".to_owned(),
            now_playing: None,
            play_status: "未播放".to_owned(),
            paused: false,
            play_started_at: None,
            play_total_ms: None,
            play_paused_at: None,
            play_paused_accum_ms: 0,
            pending_seek_ms: None,
            play_id: None,
            play_queue: PlayQueue::new(PlayMode::ListLoop),
            play_mode: PlayMode::ListLoop,
            volume: 1.0,
            play_song_id: None,
            play_error_count: 0,
            play_br: 999_000,
            crossfade_ms: 300,
            account_uid: None,
            account_nickname: None,
            playlists: Vec::new(),
            playlists_selected: 0,
            playlist_mode: PlaylistMode::List,
            playlist_tracks: Vec::new(),
            playlist_tracks_selected: 0,
            playlists_status: "等待登录后加载歌单".to_owned(),

            playlist_preloads: HashMap::new(),
            preload_summary: String::new(),

            lyrics_song_id: None,
            lyrics: Vec::new(),
            lyrics_status: "暂无歌词".to_owned(),
            lyrics_follow: true,
            lyrics_selected: 0,
            lyrics_offset_ms: 0,

            settings_selected: 0,
            settings_status: "←→ 调整 | Enter 操作 | Ctrl+Tab 切换".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppSnapshot {
    pub view: View,
    pub logged_in: bool,
    pub ui_focus: UiFocus,
    pub help_visible: bool,
    pub search_input: String,
    pub player: PlayerSnapshot,
    pub queue: Vec<Song>,
    pub queue_pos: Option<usize>,
    pub view_state: AppViewSnapshot,
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub now_playing: Option<String>,
    pub play_status: String,
    pub paused: bool,
    pub play_started_at: Option<Instant>,
    pub play_total_ms: Option<u64>,
    pub play_paused_at: Option<Instant>,
    pub play_paused_accum_ms: u64,
    pub play_mode: PlayMode,
    pub volume: f32,
    pub play_br: i64,
}

#[derive(Debug, Clone)]
pub enum AppViewSnapshot {
    Login(LoginSnapshot),
    Playlists(PlaylistsSnapshot),
    Search(SearchSnapshot),
    Lyrics(LyricsSnapshot),
    Settings(SettingsSnapshot),
}

#[derive(Debug, Clone)]
pub struct LoginSnapshot {
    pub login_qr_url: Option<String>,
    pub login_qr_ascii: Option<String>,
    pub login_status: String,
    pub login_cookie_input: String,
    pub login_cookie_input_visible: bool,
}

#[derive(Debug, Clone)]
pub struct SearchSnapshot {
    pub search_results: Vec<Song>,
    pub search_selected: usize,
    pub search_status: String,
}

#[derive(Debug, Clone)]
pub struct PlaylistsSnapshot {
    pub playlist_mode: PlaylistMode,
    pub playlists: Vec<Playlist>,
    pub playlists_selected: usize,
    pub playlist_tracks: Vec<Song>,
    pub playlist_tracks_selected: usize,
    pub playlists_status: String,
}

#[derive(Debug, Clone)]
pub struct LyricsSnapshot {
    pub lyrics: Vec<LyricLine>,
    pub lyrics_status: String,
    pub lyrics_follow: bool,
    pub lyrics_selected: usize,
    pub lyrics_offset_ms: i64,
}

#[derive(Debug, Clone)]
pub struct SettingsSnapshot {
    pub settings_selected: usize,
    pub settings_status: String,
    pub lyrics_offset_ms: i64,
    pub crossfade_ms: u64,
}

impl AppSnapshot {
    /// 从 App 创建 UI 渲染快照
    ///
    /// ## 架构说明
    ///
    /// ### 为什么需要克隆？
    /// 本函数会克隆 App 状态中的数据，这是设计上的必要权衡：
    ///
    /// 1. **线程隔离与安全**
    ///    - Core 线程（tokio）：处理业务逻辑、网络请求、音频播放
    ///    - UI 线程（ratatui）：处理渲染、用户输入
    ///    - 两个线程通过 `tokio::sync::mpsc` 通信，需要传递拥有所有权的消息
    ///
    /// 2. **避免长期引用导致的数据竞争**
    ///    - App 在 Core 线程中持续更新（播放进度、网络响应等）
    ///    - UI 渲染需要快照时间点的状态
    ///    - 如果使用引用，App 更新时可能导致 UI 读取到不一致的状态
    ///
    /// 3. **类型系统要求**
    ///    - `AppEvent::State(Box<AppSnapshot>)` 需要拥有所有权
    ///    - `mpsc::Sender` 需要发送拥有所有权的值
    ///
    /// ### 性能考虑
    ///
    /// **调用频率**：
    /// - 每次状态变化时调用一次（播放、搜索、用户操作等）
    /// - UI 刷新频率：200ms 一次（但只在状态变化时才创建新快照）
    /// - 大部分时间快照未变化，UI 只重绘相同内容
    ///
    /// **克隆开销分析**：
    /// - 小字符串（`now_playing: Option<String>`）- 开销小
    /// - `Vec<Song>`（队列、搜索结果、歌单）- 开销较大
    ///   - 典型场景：搜索结果 30 首，歌单 200 首
    ///   - Song 结构：约 50-100 字节
    ///   - 总开销：可接受范围
    ///
    /// ## 使用示例
    ///
    /// ```text
    /// // 在 Core 线程中创建快照
    /// effects.emit_state(app);
    ///
    /// // 在 UI 线程中接收快照
    /// AppEvent::State(snapshot) => {
    ///     app = *snapshot;
    /// }
    /// ```
    pub fn from_app(app: &App) -> Self {
        let player = PlayerSnapshot {
            now_playing: app.now_playing.clone(),
            play_status: app.play_status.clone(),
            paused: app.paused,
            play_started_at: app.play_started_at,
            play_total_ms: app.play_total_ms,
            play_paused_at: app.play_paused_at,
            play_paused_accum_ms: app.play_paused_accum_ms,
            play_mode: app.play_mode,
            volume: app.volume,
            play_br: app.play_br,
        };

        let view_state = match app.view {
            View::Login => AppViewSnapshot::Login(LoginSnapshot {
                login_qr_url: app.login_qr_url.clone(),
                login_qr_ascii: app.login_qr_ascii.clone(),
                login_status: app.login_status.clone(),
                login_cookie_input: app.login_cookie_input.clone(),
                login_cookie_input_visible: app.login_cookie_input_visible,
            }),
            View::Playlists => AppViewSnapshot::Playlists(PlaylistsSnapshot {
                playlist_mode: app.playlist_mode,
                playlists: if matches!(app.playlist_mode, PlaylistMode::List) {
                    app.playlists.clone()
                } else {
                    Vec::new()
                },
                playlists_selected: app.playlists_selected,
                playlist_tracks: if matches!(app.playlist_mode, PlaylistMode::Tracks) {
                    app.playlist_tracks.clone()
                } else {
                    Vec::new()
                },
                playlist_tracks_selected: app.playlist_tracks_selected,
                playlists_status: app.playlists_status.clone(),
            }),
            View::Search => AppViewSnapshot::Search(SearchSnapshot {
                search_results: app.search_results.clone(),
                search_selected: app.search_selected,
                search_status: app.search_status.clone(),
            }),
            View::Lyrics => AppViewSnapshot::Lyrics(LyricsSnapshot {
                lyrics: app.lyrics.clone(),
                lyrics_status: app.lyrics_status.clone(),
                lyrics_follow: app.lyrics_follow,
                lyrics_selected: app.lyrics_selected,
                lyrics_offset_ms: app.lyrics_offset_ms,
            }),
            View::Settings => AppViewSnapshot::Settings(SettingsSnapshot {
                settings_selected: app.settings_selected,
                settings_status: app.settings_status.clone(),
                lyrics_offset_ms: app.lyrics_offset_ms,
                crossfade_ms: app.crossfade_ms,
            }),
        };

        Self {
            view: app.view,
            logged_in: app.logged_in,
            ui_focus: app.ui_focus,
            help_visible: app.help_visible,
            search_input: app.search_input.clone(),
            player,
            queue: app.play_queue.ordered_songs(),
            queue_pos: app.play_queue.cursor_pos(),
            view_state,
        }
    }
}
