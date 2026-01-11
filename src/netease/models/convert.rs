use crate::domain::model::{Account, LoginStatus, Playlist, Song, SongUrl};

use super::dto::{
    CloudSearchResp, LoginQrCheckResp, LoginQrKeyResp, PlaylistDetailResp, SongDetailResp,
    SongUrlResp, UserAccountResp, UserPlaylistResp,
};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("缺少字段: {0}")]
    MissingField(&'static str),
    #[error("响应解析失败: {0}")]
    BadJson(#[from] serde_json::Error),
    #[error("响应为空")]
    Empty,
}

pub fn extract_unikey(resp: LoginQrKeyResp) -> Result<String, ModelError> {
    if let Some(u) = resp.unikey {
        return Ok(u);
    }
    if let Some(d) = resp.data {
        return Ok(d.unikey);
    }
    Err(ModelError::MissingField("unikey"))
}

pub fn to_login_status(resp: LoginQrCheckResp) -> LoginStatus {
    LoginStatus {
        code: resp.code,
        logged_in: resp.code == 803,
        message: resp.message,
    }
}

pub fn to_account(resp: UserAccountResp) -> Result<Account, ModelError> {
    let uid = resp.account.ok_or(ModelError::MissingField("account"))?.id;
    let nickname = resp
        .profile
        .ok_or(ModelError::MissingField("profile"))?
        .nickname;
    Ok(Account { uid, nickname })
}

pub fn to_playlists(resp: UserPlaylistResp) -> Vec<Playlist> {
    resp.playlist
        .into_iter()
        .map(|p| Playlist {
            id: p.id,
            name: p.name,
            track_count: p.track_count,
            special_type: p.special_type,
        })
        .collect()
}

pub fn to_song_list_from_search(resp: CloudSearchResp) -> Vec<Song> {
    let Some(result) = resp.result else {
        return vec![];
    };
    result.songs.into_iter().map(to_song).collect()
}

pub fn to_song_list_from_detail(resp: SongDetailResp) -> Vec<Song> {
    resp.songs.into_iter().map(to_song).collect()
}

fn to_song(s: super::dto::SongInfo) -> Song {
    let artists = if !s.ar.is_empty() { s.ar } else { s.artists };
    let artists = artists
        .into_iter()
        .map(|a| a.name)
        .collect::<Vec<_>>()
        .join("/");
    Song {
        id: s.id,
        name: s.name,
        artists,
    }
}

pub fn to_playlist_track_ids(resp: PlaylistDetailResp) -> Vec<i64> {
    resp.playlist
        .map(|p| p.track_ids.into_iter().map(|t| t.id).collect())
        .unwrap_or_default()
}

pub fn to_song_url(resp: SongUrlResp) -> Result<SongUrl, ModelError> {
    let it = resp.data.into_iter().next().ok_or(ModelError::Empty)?;
    let url = it.url.ok_or(ModelError::MissingField("data[0].url"))?;
    Ok(SongUrl { id: it.id, url })
}
