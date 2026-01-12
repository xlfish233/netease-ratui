use serde_json::Value;

use crate::domain::model::{Playlist, Song};

#[allow(dead_code)]
pub fn parse_search_songs(v: &Value) -> Vec<Song> {
    // 兼容两种常见返回：
    // - cloudsearch: {"result":{"songs":[...]}}
    // - song/detail: {"songs":[...]}}
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

#[allow(dead_code)]
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
