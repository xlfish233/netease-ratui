use crate::netease::{NeteaseClient, NeteaseClientConfig, QrPlatform};
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ApiRequest {
    EnsureAnonymous,
    LoginQrKey,
    LoginQrCheck { key: String },
    Search { keywords: String },
    SongUrl { id: i64, title: String },
    Account,
    UserPlaylists { uid: i64 },
    PlaylistTracks { playlist_id: i64 },
}

#[derive(Debug)]
pub enum ApiEvent {
    Info(String),
    Error(String),
    ClientReady { logged_in: bool },
    LoginQrReady { unikey: String, url: String, ascii: String },
    LoginQrStatus { code: i64, message: String, logged_in: bool },
    SearchResult(Value),
    SongUrlReady { id: i64, url: String, title: String },
    AccountReady { uid: i64, nickname: String },
    UserPlaylistsReady(Value),
    PlaylistTracksReady { playlist_id: i64, songs: Value },
}

pub fn spawn_api_worker(
    cfg: NeteaseClientConfig,
) -> (mpsc::Sender<ApiRequest>, mpsc::Receiver<ApiEvent>) {
    let (tx_req, mut rx_req) = mpsc::channel::<ApiRequest>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<ApiEvent>(64);

    tokio::spawn(async move {
        let mut client = match NeteaseClient::new(cfg) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx_evt.send(ApiEvent::Error(format!("初始化失败: {e}"))).await;
                return;
            }
        };

        let _ = tx_evt
            .send(ApiEvent::ClientReady {
                logged_in: client.is_logged_in(),
            })
            .await;
        if client.is_logged_in() {
            let _ = tx_evt.send(ApiEvent::Info("正在获取账号信息...".to_owned())).await;
            if let Ok(v) = client.user_account().await {
                if let Some(uid) = v.pointer("/account/id").and_then(|x| x.as_i64()) {
                    let nickname = v
                        .pointer("/profile/nickname")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_owned();
                    let _ = tx_evt.send(ApiEvent::AccountReady { uid, nickname }).await;
                }
            }
        }

        while let Some(req) = rx_req.recv().await {
            match req {
                ApiRequest::EnsureAnonymous => {
                    if let Err(e) = client.ensure_anonymous().await {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                }
                ApiRequest::LoginQrKey => match client.login_qr_key().await {
                    Ok(v) => {
                        let Some(unikey) = extract_unikey(&v) else {
                            let _ = tx_evt
                                .send(ApiEvent::Error(format!("未找到 unikey，响应={v}")))
                                .await;
                            continue;
                        };
                        let url = client.login_qr_url(unikey, QrPlatform::Pc);
                        let ascii = render_qr_ascii(&url);
                        let _ = tx_evt
                            .send(ApiEvent::LoginQrReady {
                                unikey: unikey.to_owned(),
                                url,
                                ascii,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::LoginQrCheck { key } => match client.login_qr_check(&key).await {
                    Ok(v) => {
                        let code = v.get("code").and_then(|x| x.as_i64()).unwrap_or_default();
                        let msg = v
                            .get("message")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_owned();
                        let logged_in = code == 803;
                        let _ = tx_evt
                            .send(ApiEvent::LoginQrStatus {
                                code,
                                message: msg,
                                logged_in,
                            })
                            .await;
                        if logged_in {
                            // 登录成功后拉取账号信息，便于后续获取歌单
                            let _ = tx_evt.send(ApiEvent::Info("登录成功，正在获取账号信息...".to_owned())).await;
                            match client.user_account().await {
                                Ok(acc) => {
                                    if let Some(uid) = acc.pointer("/account/id").and_then(|x| x.as_i64()) {
                                        let nickname = acc
                                            .pointer("/profile/nickname")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_owned();
                                        let _ = tx_evt.send(ApiEvent::AccountReady { uid, nickname }).await;
                                    } else {
                                        let _ = tx_evt.send(ApiEvent::Error(format!("获取账号失败，响应={acc}"))).await;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::Search { keywords } => match client.cloudsearch(&keywords, 1, 30, 0).await {
                    Ok(v) => {
                        let _ = tx_evt.send(ApiEvent::SearchResult(v)).await;
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::SongUrl { id, title } => match client.song_url(&[id], 999000).await {
                    Ok(v) => {
                        let url = v
                            .pointer("/data/0/url")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_owned();
                        if url.is_empty() {
                            let _ = tx_evt
                                .send(ApiEvent::Error(format!("未获取到播放链接，响应={v}")))
                                .await;
                            continue;
                        }
                        let _ = tx_evt
                            .send(ApiEvent::SongUrlReady { id, url, title })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::Account => match client.user_account().await {
                    Ok(v) => {
                        if let Some(uid) = v.pointer("/account/id").and_then(|x| x.as_i64()) {
                            let nickname = v
                                .pointer("/profile/nickname")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let _ = tx_evt.send(ApiEvent::AccountReady { uid, nickname }).await;
                        } else {
                            let _ = tx_evt.send(ApiEvent::Error(format!("获取账号失败，响应={v}"))).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::UserPlaylists { uid } => match client.user_playlist(uid, 200, 0).await {
                    Ok(v) => {
                        let _ = tx_evt.send(ApiEvent::UserPlaylistsReady(v)).await;
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
                ApiRequest::PlaylistTracks { playlist_id } => match client.playlist_detail(playlist_id).await {
                    Ok(v) => {
                        let ids = v
                            .pointer("/playlist/trackIds")
                            .and_then(|x| x.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|it| it.get("id").and_then(|x| x.as_i64()))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        if ids.is_empty() {
                            let _ = tx_evt.send(ApiEvent::Error(format!("歌单为空或无法解析，响应={v}"))).await;
                            continue;
                        }
                        // 最小 MVP：先取前 200 首，避免一次性过大
                        let ids = ids.into_iter().take(200).collect::<Vec<_>>();
                        match client.song_detail_by_ids(&ids).await {
                            Ok(songs) => {
                                let _ = tx_evt
                                    .send(ApiEvent::PlaylistTracksReady {
                                        playlist_id,
                                        songs,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx_evt.send(ApiEvent::Error(format!("{e}"))).await;
                    }
                },
            };
        }
    });

    (tx_req, rx_evt)
}

fn render_qr_ascii(url: &str) -> String {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return "二维码生成失败".to_owned();
    };
    code.render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build()
}

fn extract_unikey(v: &Value) -> Option<&str> {
    // 直接调用接口时，常见返回形态为：
    // - {"code":200,"unikey":"..."}
    // - {"code":200,"data":{"unikey":"..."}}
    // 兼容旧路径：/data/unikey
    v.pointer("/unikey")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/data/unikey").and_then(|x| x.as_str()))
        .or_else(|| v.pointer("/data/data/unikey").and_then(|x| x.as_str()))
}
