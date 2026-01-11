use serde_json::Value;
use std::time::Instant;

pub use crate::domain::model::{Playlist, Song};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Login,
    Playlists,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistMode {
    List,
    Tracks,
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

    pub account_uid: Option<i64>,
    pub account_nickname: Option<String>,
    pub playlists: Vec<Playlist>,
    pub playlists_selected: usize,
    pub playlist_mode: PlaylistMode,
    pub playlist_tracks: Vec<Song>,
    pub playlist_tracks_selected: usize,
    pub playlists_status: String,
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
            account_uid: None,
            account_nickname: None,
            playlists: Vec::new(),
            playlists_selected: 0,
            playlist_mode: PlaylistMode::List,
            playlist_tracks: Vec::new(),
            playlist_tracks_selected: 0,
            playlists_status: "等待登录后加载歌单".to_owned(),
        }
    }
}

pub fn parse_search_songs(v: &Value) -> Vec<Song> {
    // 兼容两种常见返回：
    // - cloudsearch: {"result":{"songs":[...]}}
    // - song/detail: {"songs":[...]}
    let songs = v
        .pointer("/result/songs")
        .or_else(|| v.pointer("/songs"))
        .and_then(|x| x.as_array());
    let Some(songs) = songs else {
        return vec![];
    };

    songs
        .iter()
        .filter_map(|s| {
            let id = s.get("id")?.as_i64()?;
            let name = s.get("name")?.as_str()?.to_owned();
            let artists = s
                .get("ar")
                .or_else(|| s.get("artists"))
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default();
            Some(Song { id, name, artists })
        })
        .collect()
}

pub fn parse_user_playlists(v: &Value) -> Vec<Playlist> {
    let Some(arr) = v.get("playlist").and_then(|x| x.as_array()) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_i64()?;
            let name = p.get("name")?.as_str()?.to_owned();
            let track_count = p.get("trackCount").and_then(|x| x.as_i64()).unwrap_or(0);
            let special_type = p.get("specialType").and_then(|x| x.as_i64()).unwrap_or(0);
            Some(Playlist {
                id,
                name,
                track_count,
                special_type,
            })
        })
        .collect()
}
