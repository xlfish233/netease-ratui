#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Song {
    pub id: i64,
    pub name: String,
    pub artists: String,
}

#[derive(Debug, Default, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub special_type: i64,
}

#[derive(Debug, Clone)]
pub struct Account {
    pub uid: i64,
    pub nickname: String,
}

#[derive(Debug, Clone)]
pub struct SongUrl {
    pub id: i64,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct LoginStatus {
    pub code: i64,
    pub message: String,
    pub logged_in: bool,
}

#[derive(Debug, Default, Clone)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    pub translation: Option<String>,
}
