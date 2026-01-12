use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LoginQrKeyResp {
    pub unikey: Option<String>,
    pub data: Option<LoginQrKeyData>,
}

#[derive(Debug, Deserialize)]
pub struct LoginQrKeyData {
    pub unikey: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginQrCheckResp {
    pub code: i64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudSearchResp {
    pub result: Option<CloudSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct CloudSearchResult {
    #[serde(default)]
    pub songs: Vec<SongInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SongDetailResp {
    #[serde(default)]
    pub songs: Vec<SongInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SongInfo {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub ar: Vec<ArtistInfo>,
    #[serde(default)]
    pub artists: Vec<ArtistInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ArtistInfo {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserAccountResp {
    pub account: Option<AccountInfo>,
    pub profile: Option<ProfileInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct ProfileInfo {
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
pub struct UserPlaylistResp {
    #[serde(default)]
    pub playlist: Vec<PlaylistInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistInfo {
    pub id: i64,
    pub name: String,
    #[serde(rename = "trackCount", default)]
    pub track_count: i64,
    #[serde(rename = "specialType", default)]
    pub special_type: i64,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistDetailResp {
    pub playlist: Option<PlaylistDetail>,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistDetail {
    #[serde(rename = "trackIds", default)]
    pub track_ids: Vec<TrackId>,
}

#[derive(Debug, Deserialize)]
pub struct TrackId {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct SongUrlResp {
    #[serde(default)]
    pub data: Vec<SongUrlItem>,
}

#[derive(Debug, Deserialize)]
pub struct SongUrlItem {
    pub id: i64,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LyricResp {
    pub lrc: Option<LyricBlock>,
    pub tlyric: Option<LyricBlock>,
}

#[derive(Debug, Deserialize)]
pub struct LyricBlock {
    #[serde(default)]
    pub lyric: String,
}
