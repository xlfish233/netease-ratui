use crate::app::{App, PlaylistMode, PlaylistPreload, PreloadStatus, View};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};
use crate::settings::{self, AppSettings};

use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

mod logout;
mod playback;
mod playlist_tracks;
mod preload;

pub fn spawn_app_actor(
    cfg: NeteaseClientConfig,
) -> (mpsc::Sender<AppCommand>, mpsc::Receiver<AppEvent>) {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<AppCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AppEvent>(64);

    let data_dir = cfg.data_dir.clone();
    let (tx_netease_hi, tx_netease_lo, mut rx_netease) =
        crate::netease::actor::spawn_netease_actor(cfg);

    // Audio worker is blocking thread + std mpsc. Bridge it to tokio mpsc.
    let (tx_audio, rx_audio) = crate::audio_worker::spawn_audio_worker(data_dir.clone());
    let (tx_audio_evt, mut rx_audio_evt) = mpsc::channel::<AudioEvent>(64);
    thread::spawn(move || {
        while let Ok(evt) = rx_audio.recv() {
            let _ = tx_audio_evt.blocking_send(evt);
        }
    });

    tokio::spawn(async move {
        let mut app = App::default();
        let mut req_id: u64 = 1;
        let mut preload_mgr = preload::PreloadManager::default();

        let mut settings = settings::load_settings(&data_dir);
        apply_settings_to_app(&mut app, &settings);

        // pending req ids to drop stale responses
        let mut pending_search: Option<u64> = None;
        let mut pending_song_url: Option<(u64, String)> = None;
        let mut pending_playlists: Option<u64> = None;
        let mut pending_playlist_detail: Option<u64> = None;
        let mut pending_playlist_tracks: Option<playlist_tracks::PlaylistTracksLoad> = None;
        let mut pending_account: Option<u64> = None;
        let mut pending_login_qr_key: Option<u64> = None;
        let mut pending_login_poll: Option<u64> = None;
        let mut pending_lyric: Option<(u64, i64)> = None;

        let mut qr_poll = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = qr_poll.tick() => {
                    if app.login_unikey.is_some()
                        && !app.logged_in
                        && let Some(key) = app.login_unikey.clone()
                    {
                        let id = next_id(&mut req_id);
                        pending_login_poll = Some(id);
                        let _ = tx_netease_hi
                            .send(NeteaseCommand::LoginQrCheck { req_id: id, key })
                            .await;
                    }
                }
                Some(cmd) = rx_cmd.recv() => {
                    match cmd {
                        AppCommand::Quit => break,
                        AppCommand::Bootstrap => {
                            app.login_status = "初始化中...".to_owned();
                            push_state(&tx_evt, &app).await;
                            let id = next_id(&mut req_id);
                            let _ = tx_netease_hi.send(NeteaseCommand::Init { req_id: id }).await;
                        }
                        AppCommand::TabNext => {
                            if app.logged_in {
                                app.view = match app.view {
                                    View::Playlists => View::Search,
                                    View::Search => View::Lyrics,
                                    View::Lyrics => View::Settings,
                                    View::Settings => View::Playlists,
                                    View::Login => View::Playlists,
                                };
                            } else {
                                app.view = match app.view {
                                    View::Login => View::Search,
                                    View::Search => View::Lyrics,
                                    View::Lyrics => View::Settings,
                                    View::Settings => View::Login,
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
                            let _ = tx_netease_hi.send(NeteaseCommand::LoginQrKey { req_id: id }).await;
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
                            let _ = tx_netease_hi.send(NeteaseCommand::CloudSearchSongs { req_id: id, keywords: q, limit: 30, offset: 0 }).await;
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
                                let _ = tx_netease_hi
                                    .send(NeteaseCommand::SongUrl {
                                        req_id: id,
                                        id: s.id,
                                        br: app.play_br,
                                    })
                                    .await;
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
                                let Some(playlist_id) = app.playlists.get(app.playlists_selected).map(|p| p.id) else {
                                    continue;
                                };

                                {
                                    if let Some(preload) = app.playlist_preloads.get(&playlist_id)
                                        && matches!(preload.status, crate::app::PreloadStatus::Completed)
                                        && !preload.songs.is_empty()
                                    {
                                        app.playlist_tracks = preload.songs.clone();
                                        app.playlist_tracks_selected = 0;
                                        app.playlist_mode = PlaylistMode::Tracks;
                                        app.queue = app.playlist_tracks.clone();
                                        app.queue_pos = Some(0);
                                        app.playlists_status =
                                            format!("歌曲: {} 首（已缓存，p 播放）", app.playlist_tracks.len());
                                        push_state(&tx_evt, &app).await;
                                        continue;
                                    }
                                }

                                    // 用户主动打开歌单：取消该歌单的预加载（若正在进行），并走高优先级加载
                                    preload_mgr.cancel_playlist(&mut app, playlist_id);

                                    app.playlists_status = "加载歌单歌曲中...".to_owned();
                                    pending_playlist_tracks = None;
                                    push_state(&tx_evt, &app).await;
                                    let id = next_id(&mut req_id);
                                    pending_playlist_detail = Some(id);
                                    let _ = tx_netease_hi
                                        .send(NeteaseCommand::PlaylistDetail {
                                            req_id: id,
                                            playlist_id,
                                        })
                                        .await;
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
                            if matches!(app.playlist_mode, PlaylistMode::Tracks)
                                && let Some(s) =
                                    app.playlist_tracks.get(app.playlist_tracks_selected)
                            {
                                app.play_status = "获取播放链接...".to_owned();
                                app.queue = app.playlist_tracks.clone();
                                app.queue_pos = Some(app.playlist_tracks_selected);
                                let title = format!("{} - {}", s.name, s.artists);
                                push_state(&tx_evt, &app).await;
                                let id = next_id(&mut req_id);
                                pending_song_url = Some((id, title));
                                let _ = tx_netease_hi
                                    .send(NeteaseCommand::SongUrl {
                                        req_id: id,
                                        id: s.id,
                                        br: app.play_br,
                                    })
                                    .await;
                            }
                        }
                        AppCommand::Back => {
                            if matches!(app.view, View::Playlists) {
                                app.playlist_mode = PlaylistMode::List;
                                pending_playlist_tracks = None;
                                pending_playlist_detail = None;
                                refresh_playlist_list_status(&mut app);
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::LyricsToggleFollow => {
                            if matches!(app.view, View::Lyrics) {
                                app.lyrics_follow = !app.lyrics_follow;
                                if app.lyrics_follow {
                                    app.lyrics_status = "歌词：跟随模式".to_owned();
                                } else {
                                    app.lyrics_status = "歌词：锁定模式（↑↓滚动，g 回到当前行）".to_owned();
                                }
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::LyricsMoveUp => {
                            if matches!(app.view, View::Lyrics)
                                && !app.lyrics_follow
                                && app.lyrics_selected > 0
                            {
                                app.lyrics_selected -= 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::LyricsMoveDown => {
                            if matches!(app.view, View::Lyrics)
                                && !app.lyrics_follow
                                && !app.lyrics.is_empty()
                                && app.lyrics_selected + 1 < app.lyrics.len()
                            {
                                app.lyrics_selected += 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::LyricsGotoCurrent => {
                            if matches!(app.view, View::Lyrics) {
                                app.lyrics_follow = true;
                                app.lyrics_status = "歌词：跟随模式".to_owned();
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::LyricsOffsetAddMs { ms } => {
                            if matches!(app.view, View::Lyrics) {
                                app.lyrics_offset_ms = app.lyrics_offset_ms.saturating_add(ms);
                                sync_settings_from_app(&mut settings, &app);
                                let _ = settings::save_settings(&data_dir, &settings);
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::PlayerTogglePause => {
                            let _ = tx_audio.send(AudioCommand::TogglePause);
                        }
                        AppCommand::PlayerStop => {
                            let _ = tx_audio.send(AudioCommand::Stop);
                        }
                        AppCommand::PlayerPrev => {
                            playback::play_prev(
                                &mut app,
                                &tx_netease_hi,
                                &mut pending_song_url,
                                &mut req_id,
                            )
                            .await;
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerNext => {
                            playback::play_next(
                                &mut app,
                                &tx_netease_hi,
                                &mut pending_song_url,
                                &mut req_id,
                            )
                            .await;
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerSeekBackwardMs { ms } => {
                            playback::seek_relative(&mut app, &tx_audio, -(ms as i64));
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerSeekForwardMs { ms } => {
                            playback::seek_relative(&mut app, &tx_audio, ms as i64);
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerVolumeDown => {
                            app.volume = (app.volume - 0.1).clamp(0.0, 2.0);
                            let _ = tx_audio.send(AudioCommand::SetVolume(app.volume));
                            sync_settings_from_app(&mut settings, &app);
                            let _ = settings::save_settings(&data_dir, &settings);
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerVolumeUp => {
                            app.volume = (app.volume + 0.1).clamp(0.0, 2.0);
                            let _ = tx_audio.send(AudioCommand::SetVolume(app.volume));
                            sync_settings_from_app(&mut settings, &app);
                            let _ = settings::save_settings(&data_dir, &settings);
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::PlayerCycleMode => {
                            app.play_mode = playback::next_play_mode(app.play_mode);
                            app.play_status =
                                format!("播放模式: {}", playback::play_mode_label(app.play_mode));
                            sync_settings_from_app(&mut settings, &app);
                            let _ = settings::save_settings(&data_dir, &settings);
                            push_state(&tx_evt, &app).await;
                        }
                        AppCommand::SettingsMoveUp => {
                            if matches!(app.view, View::Settings) && app.settings_selected > 0 {
                                app.settings_selected -= 1;
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SettingsMoveDown => {
                            if matches!(app.view, View::Settings) {
                                app.settings_selected = (app.settings_selected + 1).min(SETTINGS_ITEMS_COUNT - 1);
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SettingsDecrease => {
                            if matches!(app.view, View::Settings) {
                                apply_settings_adjust(&mut app, -1);
                                sync_settings_from_app(&mut settings, &app);
                                let _ = settings::save_settings(&data_dir, &settings);
                                let _ = tx_audio.send(AudioCommand::SetVolume(app.volume));
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SettingsIncrease => {
                            if matches!(app.view, View::Settings) {
                                apply_settings_adjust(&mut app, 1);
                                sync_settings_from_app(&mut settings, &app);
                                let _ = settings::save_settings(&data_dir, &settings);
                                let _ = tx_audio.send(AudioCommand::SetVolume(app.volume));
                                push_state(&tx_evt, &app).await;
                            }
                        }
                        AppCommand::SettingsActivate => {
                            if matches!(app.view, View::Settings) {
                                if is_clear_cache_selected(&app) {
                                    app.settings_status = "正在清除音频缓存...".to_owned();
                                    let _ = tx_audio.send(AudioCommand::ClearCache);
                                    push_state(&tx_evt, &app).await;
                                } else if is_logout_selected(&app) {
                                    if !app.logged_in {
                                        app.settings_status = "未登录，无需退出".to_owned();
                                        push_state(&tx_evt, &app).await;
                                        continue;
                                    }

                                    let _ = tx_audio.send(AudioCommand::Stop);
                                    let id = next_id(&mut req_id);
                                    let _ = tx_netease_hi
                                        .send(NeteaseCommand::LogoutLocal { req_id: id })
                                        .await;

                                    pending_search = None;
                                    pending_song_url = None;
                                    pending_playlists = None;
                                    pending_playlist_detail = None;
                                    pending_playlist_tracks = None;
                                    pending_account = None;
                                    pending_login_qr_key = None;
                                    pending_login_poll = None;
                                    pending_lyric = None;

                                    preload_mgr.reset(&mut app);
                                    logout::reset_app_after_logout(&mut app);
                                    app.login_status =
                                        "已退出登录（已清理本地 cookie），按 l 重新登录".to_owned();
                                    push_state(&tx_evt, &app).await;
                                }
                            }
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
                                let _ = tx_netease_hi.send(NeteaseCommand::UserAccount { req_id: id }).await;
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
                                let _ = tx_netease_hi.send(NeteaseCommand::UserAccount { req_id: id }).await;
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
                            let _ = tx_netease_hi.send(NeteaseCommand::UserPlaylists { req_id: id, uid: app.account_uid.unwrap_or_default() }).await;
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

                            preload_mgr
                                .start_for_playlists(&mut app, &tx_netease_lo, &mut req_id)
                                .await;

                            refresh_playlist_list_status(&mut app);
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::PlaylistTrackIds { req_id: id, playlist_id, ids } => {
                            if preload_mgr.owns_req(id)
                                && preload_mgr
                                    .on_playlist_track_ids(
                                        &mut app,
                                        &tx_netease_lo,
                                        &mut req_id,
                                        id,
                                        playlist_id,
                                        &ids,
                                    )
                                    .await
                            {
                                refresh_playlist_list_status(&mut app);
                                push_state(&tx_evt, &app).await;
                                continue;
                            }

                            if pending_playlist_detail != Some(id) {
                                continue;
                            }
                            pending_playlist_detail = None;
                            if ids.is_empty() {
                                app.playlists_status = "歌单为空或无法解析".to_owned();
                                push_state(&tx_evt, &app).await;
                                continue;
                            }

                            app.playlists_status = format!("加载歌单歌曲中... 0/{}", ids.len());
                            push_state(&tx_evt, &app).await;

                            let mut loader = playlist_tracks::PlaylistTracksLoad::new(playlist_id, ids);
                            let id = next_id(&mut req_id);
                            let chunk = loader.next_chunk();
                            loader.inflight_req_id = Some(id);
                            pending_playlist_tracks = Some(loader);
                            let _ = tx_netease_hi
                                .send(NeteaseCommand::SongDetailByIds {
                                    req_id: id,
                                    ids: chunk,
                                })
                                .await;
                        }
                        NeteaseEvent::Songs { req_id: id, songs } => {
                            if preload_mgr.owns_req(id)
                                && preload_mgr
                                    .on_songs(&mut app, &tx_netease_lo, &mut req_id, id, &songs)
                                    .await
                            {
                                refresh_playlist_list_status(&mut app);
                                push_state(&tx_evt, &app).await;
                                continue;
                            }

                            let Some(loader) = pending_playlist_tracks.as_mut() else {
                                continue;
                            };
                            if loader.inflight_req_id != Some(id) {
                                continue;
                            }
                            loader.inflight_req_id = None;
                            loader.songs.extend(songs);

                            app.playlists_status = format!(
                                "加载歌单歌曲中... {}/{}",
                                loader.songs.len(),
                                loader.total
                            );
                            push_state(&tx_evt, &app).await;

                            if loader.is_done() {
                                let loader = pending_playlist_tracks.take().expect("loader");
                                let playlist_id = loader.playlist_id;
                                let songs = loader.songs;

                                if let std::collections::hash_map::Entry::Occupied(mut entry) =
                                    app.playlist_preloads.entry(playlist_id)
                                {
                                    entry.insert(PlaylistPreload {
                                        status: PreloadStatus::Completed,
                                        songs: songs.clone(),
                                    });
                                    preload::update_preload_summary(&mut app);
                                }

                                app.playlist_tracks = songs;
                                app.playlist_tracks_selected = 0;
                                app.playlist_mode = PlaylistMode::Tracks;
                                app.queue = app.playlist_tracks.clone();
                                app.queue_pos = Some(0);
                                app.playlists_status =
                                    format!("歌曲: {} 首（p 播放）", app.playlist_tracks.len());
                                push_state(&tx_evt, &app).await;
                            } else {
                                let id = next_id(&mut req_id);
                                let chunk = loader.next_chunk();
                                loader.inflight_req_id = Some(id);
                                let _ = tx_netease_hi
                                    .send(NeteaseCommand::SongDetailByIds {
                                        req_id: id,
                                        ids: chunk,
                                    })
                                    .await;
                            }
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
                                app.play_song_id = Some(song_url.id);
                                push_state(&tx_evt, &app).await;
                                let _ = tx_audio.send(AudioCommand::PlayTrack {
                                    id: song_url.id,
                                    br: app.play_br,
                                    url: song_url.url,
                                    title,
                                });
                            }
                        }
                        NeteaseEvent::Lyric {
                            req_id: id,
                            song_id,
                            lyrics,
                        } => {
                            if pending_lyric.map(|(rid, _)| rid) != Some(id) {
                                continue;
                            }
                            pending_lyric = None;
                            app.lyrics_song_id = Some(song_id);
                            app.lyrics = lyrics;
                            app.lyrics_selected = 0;
                            app.lyrics_status = if app.lyrics.is_empty() {
                                "暂无歌词".to_owned()
                            } else {
                                format!("歌词: {} 行", app.lyrics.len())
                            };
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::LoggedOut { .. } => {}
                        NeteaseEvent::Error { req_id, message } => {
                            if preload_mgr.on_error(&mut app, req_id, message.clone()) {
                                refresh_playlist_list_status(&mut app);
                                push_state(&tx_evt, &app).await;
                                continue;
                            }

                            match app.view {
                                View::Login => app.login_status = format!("错误: {message}"),
                                View::Playlists => app.playlists_status = format!("错误: {message}"),
                                View::Search => app.search_status = format!("错误: {message}"),
                                View::Lyrics => app.lyrics_status = format!("错误: {message}"),
                                View::Settings => app.settings_status = format!("错误: {message}"),
                            }
                            push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::AnonymousReady { .. } => {}
                    }
                }
                Some(evt) = rx_audio_evt.recv() => {
                    handle_audio_event(
                        &mut app,
                        evt,
                        &tx_netease_hi,
                        &tx_audio,
                        &mut pending_song_url,
                        &mut pending_lyric,
                        &mut req_id,
                    )
                    .await;
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
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    pending_song_url: &mut Option<(u64, String)>,
    pending_lyric: &mut Option<(u64, i64)>,
    req_id: &mut u64,
) {
    match evt {
        AudioEvent::NowPlaying {
            song_id,
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
            app.play_song_id = Some(song_id);
            app.play_error_count = 0;
            let _ = tx_audio.send(AudioCommand::SetVolume(app.volume));

            app.lyrics_song_id = None;
            app.lyrics.clear();
            app.lyrics_status = "加载歌词...".to_owned();
            let id = next_id(req_id);
            *pending_lyric = Some((id, song_id));
            let _ = tx_netease
                .send(NeteaseCommand::Lyric {
                    req_id: id,
                    song_id,
                })
                .await;
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
            app.play_song_id = None;
            app.play_error_count = 0;
        }
        AudioEvent::CacheCleared { files, bytes } => {
            app.settings_status = format!(
                "已清除音频缓存：{} 个文件，释放 {} MB",
                files,
                bytes / 1024 / 1024
            );
        }
        AudioEvent::Ended { play_id } => {
            if app.play_id != Some(play_id) {
                return;
            }
            playback::play_next(app, tx_netease, pending_song_url, req_id).await;
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");

            let retryable = e.contains("下载音频失败");
            if retryable {
                app.play_error_count = app.play_error_count.saturating_add(1);
                if app.play_error_count <= 2
                    && let Some(song_id) = app.play_song_id.or_else(|| {
                        app.queue_pos
                            .and_then(|pos| app.queue.get(pos))
                            .map(|s| s.id)
                    })
                {
                    let title = if let Some(pos) = app.queue_pos {
                        app.queue
                            .get(pos)
                            .map(|s| format!("{} - {}", s.name, s.artists))
                            .unwrap_or_else(|| "未知歌曲".to_owned())
                    } else {
                        app.now_playing
                            .clone()
                            .unwrap_or_else(|| "未知歌曲".to_owned())
                    };
                    app.play_status = format!("播放失败，正在重试({}/2)...", app.play_error_count);
                    let id = next_id(req_id);
                    *pending_song_url = Some((id, title));
                    let _ = tx_netease
                        .send(NeteaseCommand::SongUrl {
                            req_id: id,
                            id: song_id,
                            br: app.play_br,
                        })
                        .await;
                }
            }
        }
    }
}

async fn push_state(tx_evt: &mpsc::Sender<AppEvent>, app: &App) {
    let _ = tx_evt.send(AppEvent::State(Box::new(app.clone()))).await;
}

const SETTINGS_ITEMS_COUNT: usize = 6;

fn apply_settings_to_app(app: &mut App, s: &AppSettings) {
    app.volume = s.volume.clamp(0.0, 2.0);
    app.play_br = s.br;
    app.play_mode = settings::play_mode_from_string(&s.play_mode);
    app.lyrics_offset_ms = s.lyrics_offset_ms;
}

fn sync_settings_from_app(s: &mut AppSettings, app: &App) {
    s.volume = app.volume;
    s.br = app.play_br;
    s.play_mode = settings::play_mode_to_string(app.play_mode);
    s.lyrics_offset_ms = app.lyrics_offset_ms;
}

fn is_logout_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 1
}

fn is_clear_cache_selected(app: &App) -> bool {
    app.settings_selected == SETTINGS_ITEMS_COUNT - 2
}

fn apply_settings_adjust(app: &mut App, dir: i32) {
    match app.settings_selected {
        0 => {
            let options = [128_000, 192_000, 320_000, 999_000];
            let pos = options
                .iter()
                .position(|v| *v == app.play_br)
                .unwrap_or(options.len() - 1);
            let next = if dir > 0 {
                (pos + 1).min(options.len() - 1)
            } else {
                pos.saturating_sub(1)
            };
            app.play_br = options[next];
            app.settings_status = format!("音质已设置为 {}", br_label(app.play_br));
        }
        1 => {
            app.volume = (app.volume + if dir > 0 { 0.05 } else { -0.05 }).clamp(0.0, 2.0);
            app.settings_status = format!("音量已设置为 {:.0}%", app.volume * 100.0);
        }
        2 => {
            app.play_mode = if dir > 0 {
                playback::next_play_mode(app.play_mode)
            } else {
                playback::prev_play_mode(app.play_mode)
            };
            app.settings_status = format!("播放模式: {}", playback::play_mode_label(app.play_mode));
        }
        3 => {
            app.lyrics_offset_ms =
                app.lyrics_offset_ms
                    .saturating_add(if dir > 0 { 200 } else { -200 });
            app.settings_status = format!("歌词 offset: {}ms", app.lyrics_offset_ms);
        }
        _ => {}
    }
}

fn br_label(br: i64) -> &'static str {
    match br {
        128_000 => "128k",
        192_000 => "192k",
        320_000 => "320k",
        999_000 => "最高",
        _ => "自定义",
    }
}

fn refresh_playlist_list_status(app: &mut App) {
    if matches!(app.view, View::Playlists) && matches!(app.playlist_mode, PlaylistMode::List) {
        let mut s = format!(
            "歌单: {} 个（已选中我喜欢的音乐，回车打开）",
            app.playlists.len()
        );
        if !app.preload_summary.is_empty() {
            s.push_str(" | ");
            s.push_str(&app.preload_summary);
        }
        app.playlists_status = s;
    }
}
