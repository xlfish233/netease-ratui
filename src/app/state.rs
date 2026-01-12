use std::collections::HashMap;
use std::time::Instant;

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
    pub play_id: Option<u64>,
    pub queue: Vec<Song>,
    pub queue_pos: Option<usize>,
    pub play_mode: PlayMode,
    pub volume: f32,
    pub play_song_id: Option<i64>,
    pub play_error_count: u32,
    pub play_br: i64,

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
            login_qr_url: None,
            login_qr_ascii: None,
            login_unikey: None,
            login_status: "按 l 生成二维码；q 退出；Tab 切换页面".to_owned(),
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
            play_id: None,
            queue: Vec::new(),
            queue_pos: None,
            play_mode: PlayMode::ListLoop,
            volume: 1.0,
            play_song_id: None,
            play_error_count: 0,
            play_br: 999_000,
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
            settings_status: "←→ 调整 | Enter 操作 | Tab 切换".to_owned(),
        }
    }
}
