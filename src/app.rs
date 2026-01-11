use serde_json::Value;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Login,
    Search,
}

#[derive(Debug, Default, Clone)]
pub struct Song {
    pub id: i64,
    pub name: String,
    pub artists: String,
}

#[derive(Debug)]
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
        }
    }
}

pub fn parse_search_songs(v: &Value) -> Vec<Song> {
    let Some(songs) = v.pointer("/result/songs").and_then(|x| x.as_array()) else {
        return vec![];
    };

    songs
        .iter()
        .filter_map(|s| {
            let id = s.get("id")?.as_i64()?;
            let name = s.get("name")?.as_str()?.to_owned();
            let artists = s
                .get("ar")
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
