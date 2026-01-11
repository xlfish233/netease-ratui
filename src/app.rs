use serde_json::Value;

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

