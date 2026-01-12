use crate::domain::model::{Account, LoginStatus, LyricLine, Playlist, Song, SongUrl};
use crate::netease::models::{convert, dto};
use crate::netease::{NeteaseClient, NeteaseClientConfig};

use serde_json::Value;
use tokio::sync::mpsc;

async fn emit_error<E: std::fmt::Display>(
    tx_evt: &mpsc::Sender<NeteaseEvent>,
    req_id: u64,
    ctx: &'static str,
    err: E,
) {
    tracing::warn!(req_id, ctx, err = %err, "NeteaseActor 请求失败");
    let _ = tx_evt
        .send(NeteaseEvent::Error {
            req_id,
            message: err.to_string(),
        })
        .await;
}

#[derive(Debug)]
pub enum NeteaseCommand {
    Init {
        req_id: u64,
    },
    #[allow(dead_code)]
    EnsureAnonymous {
        req_id: u64,
    },
    LoginQrKey {
        req_id: u64,
    },
    LoginQrCheck {
        req_id: u64,
        key: String,
    },
    UserAccount {
        req_id: u64,
    },
    UserPlaylists {
        req_id: u64,
        uid: i64,
    },
    PlaylistDetail {
        req_id: u64,
        playlist_id: i64,
    },
    SongDetailByIds {
        req_id: u64,
        ids: Vec<i64>,
    },
    CloudSearchSongs {
        req_id: u64,
        keywords: String,
        limit: i64,
        offset: i64,
    },
    SongUrl {
        req_id: u64,
        id: i64,
        br: i64,
    },
    Lyric {
        req_id: u64,
        song_id: i64,
    },
    LogoutLocal {
        req_id: u64,
    },
    LoginSetCookie {
        req_id: u64,
        music_u: String,
    },
}

#[derive(Debug)]
pub enum NeteaseEvent {
    ClientReady {
        req_id: u64,
        logged_in: bool,
    },
    AnonymousReady {
        req_id: u64,
    },
    LoginQrKey {
        req_id: u64,
        unikey: String,
    },
    LoginQrStatus {
        req_id: u64,
        status: LoginStatus,
    },
    Account {
        req_id: u64,
        account: Account,
    },
    Playlists {
        req_id: u64,
        playlists: Vec<Playlist>,
    },
    PlaylistTrackIds {
        req_id: u64,
        playlist_id: i64,
        ids: Vec<i64>,
    },
    Songs {
        req_id: u64,
        songs: Vec<Song>,
    },
    SearchSongs {
        req_id: u64,
        songs: Vec<Song>,
    },
    SongUrl {
        req_id: u64,
        song_url: SongUrl,
    },
    Lyric {
        req_id: u64,
        song_id: i64,
        lyrics: Vec<LyricLine>,
    },
    LoggedOut {
        req_id: u64,
    },
    LoginCookieSet {
        req_id: u64,
        success: bool,
        message: String,
    },
    Error {
        req_id: u64,
        message: String,
    },
}

pub fn spawn_netease_actor(
    cfg: NeteaseClientConfig,
) -> (
    mpsc::Sender<NeteaseCommand>,
    mpsc::Sender<NeteaseCommand>,
    mpsc::Receiver<NeteaseEvent>,
) {
    let (tx_hi, mut rx_hi) = mpsc::channel::<NeteaseCommand>(64);
    let (tx_lo, mut rx_lo) = mpsc::channel::<NeteaseCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<NeteaseEvent>(64);

    tokio::spawn(async move {
        let mut client = match NeteaseClient::new(cfg) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(err = %e, "初始化 NeteaseClient 失败");
                let _ = tx_evt
                    .send(NeteaseEvent::Error {
                        req_id: 0,
                        message: format!("初始化失败: {e}"),
                    })
                    .await;
                return;
            }
        };

        loop {
            let cmd = tokio::select! {
                biased;
                Some(cmd) = rx_hi.recv() => cmd,
                Some(cmd) = rx_lo.recv() => cmd,
                else => break,
            };

            match cmd {
                NeteaseCommand::Init { req_id } => {
                    let _ = tx_evt
                        .send(NeteaseEvent::ClientReady {
                            req_id,
                            logged_in: client.is_logged_in(),
                        })
                        .await;
                }
                NeteaseCommand::EnsureAnonymous { req_id } => {
                    match client.ensure_anonymous().await {
                        Ok(()) => {
                            let _ = tx_evt.send(NeteaseEvent::AnonymousReady { req_id }).await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "EnsureAnonymous", e).await;
                        }
                    }
                }
                NeteaseCommand::LoginQrKey { req_id } => match client.login_qr_key().await {
                    Ok(v) => {
                        match parse::<dto::LoginQrKeyResp>(v).and_then(convert::extract_unikey) {
                            Ok(unikey) => {
                                let _ = tx_evt
                                    .send(NeteaseEvent::LoginQrKey { req_id, unikey })
                                    .await;
                            }
                            Err(e) => {
                                emit_error(&tx_evt, req_id, "LoginQrKey(parse)", e).await;
                            }
                        }
                    }
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "LoginQrKey(request)", e).await;
                    }
                },
                NeteaseCommand::LoginQrCheck { req_id, key } => {
                    match client.login_qr_check(&key).await {
                        Ok(v) => match parse::<dto::LoginQrCheckResp>(v) {
                            Ok(v) => {
                                let status = convert::to_login_status(v);
                                let _ = tx_evt
                                    .send(NeteaseEvent::LoginQrStatus { req_id, status })
                                    .await;
                            }
                            Err(e) => {
                                emit_error(&tx_evt, req_id, "LoginQrCheck(parse)", e).await;
                            }
                        },
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "LoginQrCheck(request)", e).await;
                        }
                    }
                }
                NeteaseCommand::UserAccount { req_id } => match client.user_account().await {
                    Ok(v) => match parse::<dto::UserAccountResp>(v).and_then(convert::to_account) {
                        Ok(account) => {
                            let _ = tx_evt.send(NeteaseEvent::Account { req_id, account }).await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "UserAccount(parse)", e).await;
                        }
                    },
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "UserAccount(request)", e).await;
                    }
                },
                NeteaseCommand::UserPlaylists { req_id, uid } => {
                    match client.user_playlist(uid, 200, 0).await {
                        Ok(v) => match parse::<dto::UserPlaylistResp>(v) {
                            Ok(v) => {
                                let playlists = convert::to_playlists(v);
                                let _ = tx_evt
                                    .send(NeteaseEvent::Playlists { req_id, playlists })
                                    .await;
                            }
                            Err(e) => {
                                emit_error(&tx_evt, req_id, "UserPlaylists(parse)", e).await;
                            }
                        },
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "UserPlaylists(request)", e).await;
                        }
                    }
                }
                NeteaseCommand::PlaylistDetail {
                    req_id,
                    playlist_id,
                } => match client.playlist_detail(playlist_id).await {
                    Ok(v) => match parse::<dto::PlaylistDetailResp>(v) {
                        Ok(v) => {
                            let ids = convert::to_playlist_track_ids(v);
                            let _ = tx_evt
                                .send(NeteaseEvent::PlaylistTrackIds {
                                    req_id,
                                    playlist_id,
                                    ids,
                                })
                                .await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "PlaylistDetail(parse)", e).await;
                        }
                    },
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "PlaylistDetail(request)", e).await;
                    }
                },
                NeteaseCommand::SongDetailByIds { req_id, ids } => {
                    match client.song_detail_by_ids(&ids).await {
                        Ok(v) => match parse::<dto::SongDetailResp>(v) {
                            Ok(v) => {
                                let songs = convert::to_song_list_from_detail(v);
                                let _ = tx_evt.send(NeteaseEvent::Songs { req_id, songs }).await;
                            }
                            Err(e) => {
                                emit_error(&tx_evt, req_id, "SongDetailByIds(parse)", e).await;
                            }
                        },
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "SongDetailByIds(request)", e).await;
                        }
                    }
                }
                NeteaseCommand::CloudSearchSongs {
                    req_id,
                    keywords,
                    limit,
                    offset,
                } => match client.cloudsearch(&keywords, 1, limit, offset).await {
                    Ok(v) => match parse::<dto::CloudSearchResp>(v) {
                        Ok(v) => {
                            let songs = convert::to_song_list_from_search(v);
                            let _ = tx_evt
                                .send(NeteaseEvent::SearchSongs { req_id, songs })
                                .await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "CloudSearchSongs(parse)", e).await;
                        }
                    },
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "CloudSearchSongs(request)", e).await;
                    }
                },
                NeteaseCommand::SongUrl { req_id, id, br } => {
                    match client.song_url(&[id], br).await {
                        Ok(v) => {
                            match parse::<dto::SongUrlResp>(v).and_then(convert::to_song_url) {
                                Ok(song_url) => {
                                    let _ = tx_evt
                                        .send(NeteaseEvent::SongUrl { req_id, song_url })
                                        .await;
                                }
                                Err(e) => {
                                    emit_error(&tx_evt, req_id, "SongUrl(parse/convert)", e).await;
                                }
                            }
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "SongUrl(request)", e).await;
                        }
                    }
                }
                NeteaseCommand::Lyric { req_id, song_id } => match client.lyric(song_id).await {
                    Ok(v) => match parse::<dto::LyricResp>(v) {
                        Ok(v) => {
                            let lyrics = convert::to_lyrics(v);
                            let _ = tx_evt
                                .send(NeteaseEvent::Lyric {
                                    req_id,
                                    song_id,
                                    lyrics,
                                })
                                .await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "Lyric(parse)", e).await;
                        }
                    },
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "Lyric(request)", e).await;
                    }
                },
                NeteaseCommand::LogoutLocal { req_id } => match client.logout_local() {
                    Ok(()) => {
                        let _ = tx_evt.send(NeteaseEvent::LoggedOut { req_id }).await;
                    }
                    Err(e) => {
                        emit_error(&tx_evt, req_id, "LogoutLocal", e).await;
                    }
                },
                NeteaseCommand::LoginSetCookie { req_id, music_u } => {
                    match client.set_cookie_and_validate(&music_u).await {
                        Ok(account) => {
                            let _ = tx_evt
                                .send(NeteaseEvent::LoginCookieSet {
                                    req_id,
                                    success: true,
                                    message: format!("登录成功: {}", account.nickname),
                                })
                                .await;
                        }
                        Err(e) => {
                            emit_error(&tx_evt, req_id, "LoginSetCookie", e).await;
                        }
                    }
                }
            }
        }
    });

    (tx_hi, tx_lo, rx_evt)
}

fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, convert::ModelError> {
    serde_json::from_value(v).map_err(convert::ModelError::BadJson)
}
