use crate::app::{App, PlaylistMode, View};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};

use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

pub fn spawn_app_actor(
    cfg: NeteaseClientConfig,
) -> (mpsc::Sender<AppCommand>, mpsc::Receiver<AppEvent>) {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<AppCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AppEvent>(64);

    let (tx_netease, mut rx_netease) = crate::netease::actor::spawn_netease_actor(cfg);

    // Audio worker is blocking thread + std mpsc. Bridge it to tokio mpsc.
    let (tx_audio, rx_audio) = crate::audio_worker::spawn_audio_worker();
    let (tx_audio_evt, mut rx_audio_evt) = mpsc::channel::<AudioEvent>(64);
    thread::spawn(move || {
        while let Ok(evt) = rx_audio.recv() {
            let _ = tx_audio_evt.blocking_send(evt);
        }
    });

    tokio::spawn(async move {
        let mut app = App::default();
        let mut req_id: u64 = 1;

        // pending req ids to drop stale responses
        let mut pending_search: Option<u64> = None;
        let mut pending_song_url: Option<(u64, String)> = None;
        let mut pending_playlists: Option<u64> = None;
        let mut pending_playlist_tracks: Option<(u64, i64)> = None; // (songs_req_id, playlist_id)
        let mut pending_account: Option<u64> = None;
        let mut pending_login_qr_key: Option<u64> = None;
        let mut pending_login_poll: Option<u64> = None;

        let mut qr_poll = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = qr_poll.tick() => {
                    if app.login_unikey.is_some() && !app.logged_in {
                        if let Some(key) = app.login_unikey.clone() {
                            let id = next_id(&mut req_id);
                            pending_login_poll = Some(id);
                            let _ = tx_netease.send(NeteaseCommand::LoginQrCheck { req_id: id, key }).await;
                        }
                    }
                }
                Some(cmd) = rx_cmd.recv() => {
                    match cmd {
                        AppCommand::Quit => break,
                        AppCommand::Bootstrap => {
                            app.login_status = "初始化中...".to_owned();
                            push_state(&tx_evt, &app).await;
                            let id = next_id(&mut req_id);
                            let _ = tx_netease.send(NeteaseCommand::Init { req_id: id }).await;
                        }
                        AppCommand::TabNext => {
                            if app.logged_in {
                                app.view = match app.view {
                                    View::Playlists => View::Search,
                                    View::Search => View::Playlists,
                                    View::Login => View::Playlists,
                                };
                            } else {
                                app.view = match app.view {
                                    View::Login => View::Search,
                                    View::Search => View::Login,
                                    View::Playlists => View::Login,
                                };
                            }
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::LoginGenerateQr => {
                            if app.logged_in {
                                continue;
                            }
                            app.login_status = "正在生成二维码...".to_owned();
                            push_state(&tx_evt, &app).await;
                            let id = next_id(&mut req_id);
                            pending_login_qr_key = Some(id);
                            let _ = tx_netease.send(NeteaseCommand::LoginQrKey { req_id: id }).await;
                        }
                        AppCommand::SearchSubmit => {
                            let q = app.search_input.trim().to_owned();
                            if q.is_empty() {
                                app.search_status = "请输入关键词".to_owned();
                                push_state(&tx_evt, &app).await;
                                continue;
                            }
                            app.search_status = "搜索中...".to_owned();
                            app.search_results.clear();
                            app.search_selected = 0;
                            push_state(&tx_evt, &app).await;
                            let id = next_id(&mut req_id);
                            pending_search = Some(id);
                            let _ = tx_netease.send(NeteaseCommand::CloudSearchSongs { req_id: id, keywords: q, limit: 30, offset: 0 }).await;
                        }
                        AppCommand::SearchInputBackspace => {
                            app.search_input.pop();
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::SearchInputChar { c } => {
                            app.search_input.push(c);
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::SearchMoveUp => {
                            if app.search_selected > 0 {
                                app.search_selected -= 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SearchMoveDown => {
                            if !app.search_results.is_empty() && app.search_selected + 1 < app.search_results.len() {
                                app.search_selected += 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SearchPlaySelected => {
                            if let Some(s) = app.search_results.get(app.search_selected) {
                                app.play_status = "获取播放链接...".to_owned();
                                app.queue.clear();
                                app.queue_pos = None;
                                let title = format!("{} - {}", s.name, s.artists);
                                push_state(&tx_evt, &app).await;
                                let id = next_id(&mut req_id);
                                pending_song_url = Some((id, title));
                                let _ = tx_netease.send(NeteaseCommand::SongUrl { req_id: id, id: s.id, br: 999000 }).await;
                            }
                        }
                        AppCommand::PlaylistsMoveUp => {
                            if app.playlists_selected > 0 {
                                app.playlists_selected -= 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlaylistsMoveDown => {
                            if !app.playlists.is_empty() && app.playlists_selected + 1 < app.playlists.len() {
                                app.playlists_selected += 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlaylistsOpenSelected => {
                            if matches!(app.playlist_mode, PlaylistMode::List) {
                                if let Some(p) = app.playlists.get(app.playlists_selected) {
                                    app.playlists_status = "加载歌单歌曲中...".to_owned();
                                    push_state(&tx_evt, &app).await;
                                    let id = next_id(&mut req_id);
                                    let _ = tx_netease.send(NeteaseCommand::PlaylistDetail { req_id: id, playlist_id: p.id }).await;
                                    // playlist track ids is an intermediate; we'll store pending when the songs request is sent.
                                }
                            }
                        }
                        AppCommand::PlaylistTracksMoveUp => {
                            if app.playlist_tracks_selected > 0 {
                                app.playlist_tracks_selected -= 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlaylistTracksMoveDown => {
                            if !app.playlist_tracks.is_empty() && app.playlist_tracks_selected + 1 < app.playlist_tracks.len() {
                                app.playlist_tracks_selected += 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlaylistTracksPlaySelected => {
                            if matches!(app.playlist_mode, PlaylistMode::Tracks) {
                                if let Some(s) = app.playlist_tracks.get(app.playlist_tracks_selected) {
                                    app.play_status = "获取播放链接...".to_owned();
                                    app.queue = app.playlist_tracks.clone();
                                    app.queue_pos = Some(app.playlist_tracks_selected);
                                    let title = format!("{} - {}", s.name, s.artists);
                                    push_state(&tx_evt, &app).await;
                                    let id = next_id(&mut req_id);
                                    pending_song_url = Some((id, title));
                                    let _ = tx_netease.send(NeteaseCommand::SongUrl { req_id: id, id: s.id, br: 999000 }).await;
                                }
                            }
                        }
                        AppCommand::Back => {
                            if matches!(app.view, View::Playlists) {
                                app.playlist_mode = PlaylistMode::List;
                                app.playlists_status = "返回歌单列表".to_owned();
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlayerTogglePause => {
                            let _ = tx_audio.send(AudioCommand::TogglePause);
                        }
                        AppCommand::PlayerStop => {
                            let _ = tx_audio.send(AudioCommand::Stop);
                        }
                    }
                }
                Some(evt) = rx_netease.recv() => {
                    match evt {
                        NeteaseEvent::ClientReady { req_id: _, logged_in } => {
                            app.logged_in = logged_in;
                            if app.logged_in {
                                app.view = View::Playlists;
                                app.playlists_status = "已登录（已从本地状态恢复），正在加载账号信息...".to_owned();
                                push_state(&tx_evt, &app).await;
                                let id = next_id(&mut req_id);
                                pending_account = Some(id);
                                let _ = tx_netease.send(NeteaseCommand::UserAccount { req_id: id }).await;
                            } else {
                                app.login_status = "按 l 生成二维码；q 退出；Tab 切换页面".to_owned();
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        NeteaseEvent::LoginQrKey { req_id: id, unikey } => {
                            if pending_login_qr_key != Some(id) {
                                continue;
                            }
                            app.login_unikey = Some(unikey.clone());
                            app.login_qr_url =
                                Some(format!("https://music.163.com/login?codekey={unikey}"));
                            // Build QR ASCII locally; keep previous helper style for now.
                            app.login_qr_ascii = Some(render_qr_ascii(app.login_qr_url.as_deref().unwrap_or("")));
                            app.login_status = "请用网易云 APP 扫码；扫码后会自动轮询状态".to_owned();
                            app.logged_in = false;
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::LoginQrStatus { req_id: id, status } => {
                            if pending_login_poll != Some(id) {
                                continue;
                            }
                            if status.logged_in {
                                app.logged_in = true;
                                app.login_status = "登录成功".to_owned();
                                app.view = View::Playlists;
                                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                                push_state(&tx_evt, &app).await;
                                let id = next_id(&mut req_id);
                                pending_account = Some(id);
                                let _ = tx_netease.send(NeteaseCommand::UserAccount { req_id: id }).await;
                            } else {
                                app.login_status = format!("扫码状态 code={} {}", status.code, status.message);
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        NeteaseEvent::Account { req_id: id, account } => {
                            if pending_account != Some(id) {
                                continue;
                            }
                            app.account_uid = Some(account.uid);
                            app.account_nickname = Some(account.nickname);
                            app.playlists_status = "正在加载用户歌单...".to_owned();
                            push_state(&tx_evt, &app).await;
                            let id = next_id(&mut req_id);
                            pending_playlists = Some(id);
                            let _ = tx_netease.send(NeteaseCommand::UserPlaylists { req_id: id, uid: app.account_uid.unwrap_or_default() }).await;
                        }
                        NeteaseEvent::Playlists { req_id: id, playlists } => {
                            if pending_playlists != Some(id) {
                                continue;
                            }
                            app.playlists = playlists;
                            app.playlists_selected = app
                                .playlists
                                .iter()
                                .position(|p| p.special_type == 5 || p.name.contains("我喜欢"))
                                .unwrap_or(0);
                            app.playlist_mode = PlaylistMode::List;
                            app.playlist_tracks.clear();
                            app.playlist_tracks_selected = 0;
                            app.playlists_status = format!("歌单: {} 个（已选中我喜欢的音乐，回车打开）", app.playlists.len());
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::PlaylistTrackIds { req_id: _, playlist_id, ids } => {
                            if ids.is_empty() {
                                app.playlists_status = "歌单为空或无法解析".to_owned();
                                push_state(&tx_evt, &app).await;
                                continue;
                            }
                            let ids = ids.into_iter().take(200).collect::<Vec<_>>();
                            let id = next_id(&mut req_id);
                            pending_playlist_tracks = Some((id, playlist_id));
                            let _ = tx_netease.send(NeteaseCommand::SongDetailByIds { req_id: id, ids }).await;
                        }
                        NeteaseEvent::Songs { req_id: id, songs } => {
                            let Some((pending_id, _playlist_id)) = pending_playlist_tracks else { continue; };
                            if pending_id != id {
                                continue;
                            }
                            app.playlist_tracks = songs;
                            app.playlist_tracks_selected = 0;
                            app.playlist_mode = PlaylistMode::Tracks;
                            app.queue = app.playlist_tracks.clone();
                            app.queue_pos = Some(0);
                            app.playlists_status = format!("歌曲: {} 首（p 播放）", app.playlist_tracks.len());
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::SearchSongs { req_id: id, songs } => {
                            if pending_search != Some(id) {
                                continue;
                            }
                            app.search_results = songs;
                            app.search_selected = 0;
                            app.search_status = format!("结果: {} 首", app.search_results.len());
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::SongUrl { req_id: id, song_url } => {
                            if let Some((pending_id, title)) = pending_song_url.take() {
                                if pending_id != id {
                                    continue;
                                }
                                app.play_status = "开始播放...".to_owned();
                                push_state(&tx_evt, &app).await;
                                let _ = tx_audio.send(AudioCommand::PlayUrl { url: song_url.url, title });
                            }
                        }
                        NeteaseEvent::Error { req_id: _, message } => {
                            match app.view {
                                View::Login => app.login_status = format!("错误: {message}"),
                                View::Playlists => app.playlists_status = format!("错误: {message}"),
                                View::Search => app.search_status = format!("错误: {message}"),
                            }
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::AnonymousReady { .. } => {}
                    }
                }
                Some(evt) = rx_audio_evt.recv() => {
                    handle_audio_event(&mut app, evt, &tx_netease, &mut pending_song_url, &mut req_id).await;
                    push_state(&tx_evt, &app).await;
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}

fn next_id(id: &mut u64) -> u64 {
    let out = *id;
    *id = id.wrapping_add(1);
    out
}

fn render_qr_ascii(url: &str) -> String {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return "二维码生成失败".to_owned();
    };
    code.render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build()
}

async fn handle_audio_event(
    app: &mut App,
    evt: AudioEvent,
    tx_netease: &mpsc::Sender<NeteaseCommand>,
    pending_song_url: &mut Option<(u64, String)>,
    req_id: &mut u64,
) {
    match evt {
        AudioEvent::NowPlaying {
            play_id,
            title,
            duration_ms,
        } => {
            app.now_playing = Some(title);
            app.paused = false;
            app.play_status = "播放中".to_owned();
            app.play_started_at = Some(std::time::Instant::now());
            app.play_total_ms = duration_ms;
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = Some(play_id);
        }
        AudioEvent::Paused(p) => {
            app.paused = p;
            app.play_status = if p { "已暂停" } else { "播放中" }.to_owned();
            if p {
                app.play_paused_at = Some(std::time::Instant::now());
            } else if let Some(t) = app.play_paused_at.take() {
                app.play_paused_accum_ms = app
                    .play_paused_accum_ms
                    .saturating_add(t.elapsed().as_millis() as u64);
            }
        }
        AudioEvent::Stopped => {
            app.paused = false;
            app.play_status = "已停止".to_owned();
            app.play_started_at = None;
            app.play_total_ms = None;
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = None;
        }
        AudioEvent::Ended { play_id } => {
            if app.play_id != Some(play_id) {
                return;
            }
            let Some(pos) = app.queue_pos else {
                return;
            };
            let next = pos + 1;
            if next >= app.queue.len() {
                app.play_status = "播放结束".to_owned();
                app.queue_pos = None;
                return;
            }
            app.queue_pos = Some(next);
            if matches!(app.view, View::Playlists)
                && matches!(app.playlist_mode, PlaylistMode::Tracks)
            {
                app.playlist_tracks_selected =
                    next.min(app.playlist_tracks.len().saturating_sub(1));
            }
            let s = &app.queue[next];
            app.play_status = "自动下一首...".to_owned();
            let title = format!("{} - {}", s.name, s.artists);
            let id = next_id(req_id);
            *pending_song_url = Some((id, title));
            let _ = tx_netease
                .send(NeteaseCommand::SongUrl {
                    req_id: id,
                    id: s.id,
                    br: 999000,
                })
                .await;
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");
        }
    }
}

async fn push_state(tx_evt: &mpsc::Sender<AppEvent>, app: &App) {
    let _ = tx_evt.send(AppEvent::State(app.clone())).await;
}
