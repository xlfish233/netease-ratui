use crate::app::{App, View, tab_configs};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};
use crate::settings;

use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

mod audio_handler;
mod login;
mod logout;
mod lyrics;
mod playback;
mod player_control;
mod playlist_tracks;
mod playlists;
mod preload;
mod search;
mod settings_handler;
mod utils;

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
        settings_handler::apply_settings_to_app(&mut app, &settings);

        // pending req ids to drop stale responses
        let mut pending_search: Option<u64> = None;
        let mut pending_song_url: Option<(u64, String)> = None;
        let mut pending_playlists: Option<u64> = None;
        let mut pending_playlist_detail: Option<u64> = None;
        let mut pending_playlist_tracks: Option<playlist_tracks::PlaylistTracksLoad> = None;
        let mut pending_account: Option<u64> = None;
        let mut pending_login_qr_key: Option<u64> = None;
        let mut pending_login_poll: Option<u64> = None;
        let mut pending_login_set_cookie: Option<u64> = None;
        let mut pending_lyric: Option<(u64, i64)> = None;

        let mut qr_poll = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = qr_poll.tick() => {
                    if let Some(key) = app.login_unikey.as_ref().filter(|_| !app.logged_in) {
                        let id = utils::next_id(&mut req_id);
                        pending_login_poll = Some(id);
                        if let Err(e) = tx_netease_hi
                            .send(NeteaseCommand::LoginQrCheck { req_id: id, key: key.clone() })
                            .await
                        {
                            tracing::warn!(err = %e, "NeteaseActor 通道已关闭：LoginQrCheck 发送失败");
                        }
                    }
                }
                Some(cmd) = rx_cmd.recv() => {
                    match cmd {
                        AppCommand::Quit => break,
                        AppCommand::Bootstrap => {
                            app.login_status = "初始化中...".to_owned();
                            utils::push_state(&tx_evt, &app).await;
                            let id = utils::next_id(&mut req_id);
                            if let Err(e) =
                                tx_netease_hi.send(NeteaseCommand::Init { req_id: id }).await
                            {
                                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：Init 发送失败");
                            }
                        }
                        AppCommand::TabNext => {
                            let configs = tab_configs(app.logged_in);
                            let current_idx = configs
                                .iter()
                                .position(|c| c.view == app.view)
                                .unwrap_or(0);
                            let next_view = configs[(current_idx + 1) % configs.len()].view;
                            app.view = next_view;
                            utils::push_state(&tx_evt, &app).await;
                        }
                        AppCommand::TabTo { index } => {
                            if let Some(&cfg) = tab_configs(app.logged_in).get(index) {
                                app.view = cfg.view;
                                utils::push_state(&tx_evt, &app).await;
                            }
                        }
                        cmd @ (AppCommand::LoginGenerateQr
                            | AppCommand::LoginToggleCookieInput
                            | AppCommand::LoginCookieInputChar { .. }
                            | AppCommand::LoginCookieInputBackspace
                            | AppCommand::LoginCookieSubmit) => {
                            if login::handle_login_command(
                                cmd,
                                &mut app,
                                &mut req_id,
                                &mut pending_login_qr_key,
                                &mut pending_login_set_cookie,
                                &tx_netease_hi,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        cmd @ (AppCommand::SearchSubmit
                            | AppCommand::SearchInputBackspace
                            | AppCommand::SearchInputChar { .. }
                            | AppCommand::SearchMoveUp
                            | AppCommand::SearchMoveDown
                            | AppCommand::SearchPlaySelected) => {
                            if search::handle_search_command(
                                cmd,
                                &mut app,
                                &mut req_id,
                                &mut pending_search,
                                &mut pending_song_url,
                                &tx_netease_hi,
                                &tx_audio,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        | cmd @ (AppCommand::PlaylistsMoveUp
                            | AppCommand::PlaylistsMoveDown
                            | AppCommand::PlaylistsOpenSelected
                            | AppCommand::PlaylistTracksMoveUp
                            | AppCommand::PlaylistTracksMoveDown
                            | AppCommand::PlaylistTracksPlaySelected) => {
                            if playlists::handle_playlists_command(
                                cmd,
                                &mut app,
                                &mut req_id,
                                &mut pending_song_url,
                                &mut pending_playlist_detail,
                                &mut pending_playlist_tracks,
                                &mut preload_mgr,
                                &tx_netease_hi,
                                &tx_audio,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        AppCommand::Back => {
                            if playlists::handle_playlists_back_command(
                                cmd,
                                &mut app,
                                &mut pending_playlist_detail,
                                &mut pending_playlist_tracks,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        | cmd @ (AppCommand::LyricsToggleFollow
                            | AppCommand::LyricsMoveUp
                            | AppCommand::LyricsMoveDown
                            | AppCommand::LyricsGotoCurrent
                            | AppCommand::LyricsOffsetAddMs { .. }) => {
                            lyrics::handle_lyrics_command(
                                cmd,
                                &mut app,
                                &mut settings,
                                &data_dir,
                                &tx_evt,
                            )
                            .await;
                        }
                        | cmd @ (AppCommand::PlayerTogglePause
                            | AppCommand::PlayerStop
                            | AppCommand::PlayerPrev
                            | AppCommand::PlayerNext
                            | AppCommand::PlayerSeekBackwardMs { .. }
                            | AppCommand::PlayerSeekForwardMs { .. }) => {
                            player_control::handle_player_control_command(
                                cmd,
                                &mut app,
                                &mut req_id,
                                &mut pending_song_url,
                                &tx_audio,
                                &tx_netease_hi,
                                &tx_evt,
                            )
                            .await;
                        }
                        | cmd @ (AppCommand::PlayerVolumeDown
                            | AppCommand::PlayerVolumeUp
                            | AppCommand::PlayerCycleMode) => {
                            settings_handler::handle_player_settings_command(
                                cmd,
                                &mut app,
                                &mut settings,
                                &data_dir,
                                &tx_audio,
                                &tx_evt,
                            )
                            .await;
                        }
                        | cmd @ (AppCommand::SettingsMoveUp
                            | AppCommand::SettingsMoveDown
                            | AppCommand::SettingsDecrease
                            | AppCommand::SettingsIncrease) => {
                            settings_handler::handle_settings_command(
                                cmd,
                                &mut app,
                                &mut settings,
                                &data_dir,
                                &tx_audio,
                                &tx_evt,
                            )
                            .await;
                        }
                        AppCommand::SettingsActivate => {
                            match settings_handler::handle_settings_activate_command(
                                &mut app,
                                &tx_audio,
                                &tx_evt,
                            )
                            .await
                            {
                                Some(true) => continue, // 已处理且应 continue
                                Some(false) => {}     // 未处理（继续处理登出）
                                None => {}
                            }
                            // 登出处理逻辑保持不变
                            if !app.logged_in {
                                app.settings_status = "未登录，无需退出".to_owned();
                                utils::push_state(&tx_evt, &app).await;
                                continue;
                            }

                            tracing::info!("用户触发：退出登录");
                            if tx_audio.send(AudioCommand::Stop).is_err() {
                                tracing::warn!("AudioWorker 通道已关闭：Stop 发送失败");
                            }
                            let id = utils::next_id(&mut req_id);
                            if let Err(e) = tx_netease_hi
                                .send(NeteaseCommand::LogoutLocal { req_id: id })
                                .await
                            {
                                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：LogoutLocal 发送失败");
                            }

                            pending_search = None;
                            pending_song_url = None;
                            pending_playlists = None;
                            pending_playlist_detail = None;
                            pending_playlist_tracks = None;
                            pending_account = None;
                            pending_login_qr_key = None;
                            pending_login_poll = None;
                            pending_login_set_cookie = None;
                            pending_lyric = None;

                            preload_mgr.reset(&mut app);
                            logout::reset_app_after_logout(&mut app);
                            app.login_status =
                                "已退出登录（已清理本地 cookie），按 l 重新登录".to_owned();
                            utils::push_state(&tx_evt, &app).await;
                        }
                    }
                }
                Some(evt) = rx_netease.recv() => {
                    // 处理登录相关事件
                    if login::handle_login_event(
                        &evt,
                        &mut app,
                        &mut req_id,
                        &mut pending_login_qr_key,
                        &mut pending_login_poll,
                        &mut pending_login_set_cookie,
                        &mut pending_account,
                        &tx_netease_hi,
                        &tx_evt,
                    )
                    .await
                    {
                        continue;
                    }

                    match evt {
                        NeteaseEvent::Account { req_id: id, account } => {
                            if pending_account != Some(id) {
                                continue;
                            }
                            app.account_uid = Some(account.uid);
                            app.account_nickname = Some(account.nickname);
                            app.playlists_status = "正在加载用户歌单...".to_owned();
                            utils::push_state(&tx_evt, &app).await;
                            let id = utils::next_id(&mut req_id);
                            pending_playlists = Some(id);
                            if let Err(e) = tx_netease_hi
                                .send(NeteaseCommand::UserPlaylists {
                                    req_id: id,
                                    uid: app.account_uid.unwrap_or_default(),
                                })
                                .await
                            {
                                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：UserPlaylists 发送失败");
                            }
                        }
                        NeteaseEvent::Playlists { req_id: id, playlists } => {
                            if !playlists::handle_playlists_event(
                                id,
                                playlists,
                                &mut app,
                                &mut pending_playlists,
                                &mut preload_mgr,
                                &tx_netease_lo,
                                &mut req_id,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        NeteaseEvent::PlaylistTrackIds { req_id: id, playlist_id, ids } => {
                            // 先检查预加载管理器是否拥有此请求
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
                                playlists::refresh_playlist_list_status(&mut app);
                                utils::push_state(&tx_evt, &app).await;
                                continue;
                            }

                            match playlists::handle_playlist_detail_event(
                                id,
                                playlist_id,
                                ids,
                                &mut app,
                                &mut pending_playlist_detail,
                                &mut pending_playlist_tracks,
                                &preload_mgr,
                                &tx_netease_hi,
                                &mut req_id,
                                &tx_evt,
                            )
                            .await
                            {
                                Some(true) => continue, // 已处理且应 continue
                                Some(false) => {}     // 未处理（预加载管理器处理）
                                None => continue,     // req_id 不匹配
                            }
                        }
                        NeteaseEvent::Songs { req_id: id, songs } => {
                            // 先检查预加载管理器是否拥有此请求
                            if preload_mgr.owns_req(id)
                                && preload_mgr
                                    .on_songs(&mut app, &tx_netease_lo, &mut req_id, id, &songs)
                                    .await
                            {
                                playlists::refresh_playlist_list_status(&mut app);
                                utils::push_state(&tx_evt, &app).await;
                                continue;
                            }

                            match playlists::handle_songs_event(
                                id,
                                songs,
                                &mut app,
                                &mut pending_playlist_tracks,
                                &mut preload_mgr,
                                &tx_netease_hi,
                                &mut req_id,
                                &tx_evt,
                            )
                            .await
                            {
                                Some(true) => continue, // 已处理且应 continue
                                Some(false) => {}     // 未处理
                                None => {}            // 不应该发生
                            }
                        }
                        NeteaseEvent::SearchSongs { req_id: id, songs } => {
                            if !search::handle_search_songs_event(
                                id,
                                &songs,
                                &mut app,
                                &mut pending_search,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        NeteaseEvent::SongUrl { req_id: id, song_url } => {
                            if let Some((pending_id, title)) = pending_song_url.take() {
                                if pending_id != id {
                                    continue;
                                }
                                app.play_status = "开始播放...".to_owned();
                                app.play_song_id = Some(song_url.id);
                                utils::push_state(&tx_evt, &app).await;
                                if tx_audio
                                    .send(AudioCommand::PlayTrack {
                                        id: song_url.id,
                                        br: app.play_br,
                                        url: song_url.url,
                                        title,
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("AudioWorker 通道已关闭：PlayTrack 发送失败");
                                }
                            }
                        }
                        NeteaseEvent::Lyric {
                            req_id: id,
                            song_id,
                            lyrics,
                        } => {
                            if !lyrics::handle_lyric_event(
                                id,
                                song_id,
                                lyrics,
                                &mut app,
                                &mut pending_lyric,
                                &tx_evt,
                            )
                            .await
                            {
                                continue;
                            }
                        }
                        NeteaseEvent::LoggedOut { req_id } => {
                            tracing::debug!(req_id, "NeteaseActor: LoggedOut");
                        }
                        NeteaseEvent::Error { req_id, message } => {
                            if preload_mgr.on_error(&mut app, req_id, &message) {
                                playlists::refresh_playlist_list_status(&mut app);
                                utils::push_state(&tx_evt, &app).await;
                                continue;
                            }

                            match app.view {
                                View::Login => app.login_status = format!("错误: {message}"),
                                View::Playlists => app.playlists_status = format!("错误: {message}"),
                                View::Search => app.search_status = format!("错误: {message}"),
                                View::Lyrics => app.lyrics_status = format!("错误: {message}"),
                                View::Settings => app.settings_status = format!("错误: {message}"),
                            }
                            utils::push_state(&tx_evt, &app).await;
                        }
                        NeteaseEvent::AnonymousReady { req_id } => {
                            tracing::debug!(req_id, "NeteaseActor: AnonymousReady");
                        }
                        _ => {}
                    }
                }
                Some(evt) = rx_audio_evt.recv() => {
                    audio_handler::handle_audio_event(
                        &mut app,
                        evt,
                        &tx_netease_hi,
                        &tx_audio,
                        &mut pending_song_url,
                        &mut pending_lyric,
                        &mut req_id,
                    )
                    .await;
                    utils::push_state(&tx_evt, &app).await;
                }
            }
        }
    });

    (tx_cmd, rx_evt)
}
