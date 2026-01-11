use crate::domain::model::{Account, LoginStatus, LyricLine, Playlist, Song, SongUrl};

use super::dto::{
    CloudSearchResp, LoginQrCheckResp, LoginQrKeyResp, LyricResp, PlaylistDetailResp,
    SongDetailResp, SongUrlResp, UserAccountResp, UserPlaylistResp,
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

pub fn to_lyrics(resp: LyricResp) -> Vec<LyricLine> {
    let original = resp
        .lrc
        .map(|b| parse_lrc_original(&b.lyric))
        .unwrap_or_default();
    let translation = resp
        .tlyric
        .map(|b| parse_lrc_translation(&b.lyric))
        .unwrap_or_default();

    if translation.is_empty() {
        return original;
    }

    let mut trans_map = std::collections::HashMap::<u64, String>::new();
    for it in translation {
        trans_map.entry(it.time_ms).or_insert(it.text);
    }

    original
        .into_iter()
        .map(|mut l| {
            if let Some(t) = trans_map.get(&l.time_ms) {
                l.translation = Some(t.clone());
            }
            l
        })
        .collect()
}

fn parse_lrc_original(text: &str) -> Vec<LyricLine> {
    parse_lrc_text(text, false)
        .into_iter()
        .filter_map(|(time_ms, content)| {
            if content.trim().is_empty() {
                return None;
            }
            Some(LyricLine {
                time_ms,
                text: content,
                translation: None,
            })
        })
        .collect()
}

fn parse_lrc_translation(text: &str) -> Vec<LyricLine> {
    parse_lrc_text(text, true)
        .into_iter()
        .map(|(time_ms, content)| LyricLine {
            time_ms,
            text: content,
            translation: None,
        })
        .collect()
}

fn parse_lrc_text(text: &str, allow_empty_text: bool) -> Vec<(u64, String)> {
    let mut out = Vec::new();

    for line in text.lines() {
        let mut rest = line.trim();
        if rest.is_empty() {
            continue;
        }

        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else {
                break;
            };
            let tag = &stripped[..end];
            rest = &stripped[end + 1..];
            if let Some(t) = parse_lrc_timestamp_ms(tag) {
                times.push(t);
            }
        }

        let content = rest.trim();
        if content.is_empty() && !allow_empty_text {
            continue;
        }

        for t in times {
            out.push((t, content.to_owned()));
        }
    }

    out.sort_by_key(|(t, _)| *t);
    out
}

fn parse_lrc_timestamp_ms(tag: &str) -> Option<u64> {
    // mm:ss.xx or mm:ss.xxx
    let (mm, rest) = tag.split_once(':')?;
    let mm: u64 = mm.parse().ok()?;
    let (ss, frac) = rest
        .split_once('.')
        .map(|(a, b)| (a, b))
        .unwrap_or((rest, ""));
    let ss: u64 = ss.parse().ok()?;
    let frac_digits = frac
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .take(3)
        .collect::<String>();
    let frac_val: u64 = if frac_digits.is_empty() {
        0
    } else {
        frac_digits.parse().ok()?
    };
    let frac_ms = match frac_digits.len() {
        0 => 0,
        1 => frac_val * 100,
        2 => frac_val * 10,
        _ => frac_val,
    };
    Some(mm * 60_000 + ss * 1_000 + frac_ms)
}
