use crate::app::{App, View, tab_configs};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};
use crate::settings;

use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::core::effects::{CoreDispatch, CoreEffects, run_effects};
use crate::core::infra::{NextSongCacheManager, PreloadManager, RequestKey, RequestTracker};
use crate::core::utils;

use crate::features::login;
use crate::features::logout;
use crate::features::lyrics;
use crate::features::player;
use crate::features::playlists;
use crate::features::playlists::PlaylistTracksLoad;
use crate::features::search;
use crate::features::settings as settings_handlers;

enum CoreMsg {
    Ui(AppCommand),
    Netease(NeteaseEvent),
    Audio(AudioEvent),
    QrPoll,
}

struct CoreState {
    app: App,
    req_id: u64,
    preload_mgr: PreloadManager,
    next_song_cache: NextSongCacheManager,
    settings: settings::AppSettings,
    request_tracker: RequestTracker<RequestKey>,
    pending_song_url: Option<(u64, String)>,
    pending_playlists: Option<u64>,
    pending_playlist_detail: Option<u64>,
    pending_playlist_tracks: Option<PlaylistTracksLoad>,
    pending_lyric: Option<(u64, i64)>,
}

#[allow(clippy::too_many_arguments)]
async fn reduce(
    msg: CoreMsg,
    state: &mut CoreState,
    effects: &mut CoreEffects,
    data_dir: &std::path::Path,
) -> bool {
    match msg {
        CoreMsg::QrPoll => {
            login::handle_qr_poll(
                &state.app,
                &mut state.req_id,
                &mut state.request_tracker,
                effects,
            );
        }
        CoreMsg::Ui(cmd) => match cmd {
            AppCommand::Quit => return true,
            AppCommand::Bootstrap => {
                state.app.login_status = "初始化中...".to_owned();
                effects.emit_state(&state.app);
                let id = utils::next_id(&mut state.req_id);
                effects.send_netease_hi_warn(
                    NeteaseCommand::Init { req_id: id },
                    "NeteaseActor 通道已关闭：Init 发送失败",
                );
            }
            AppCommand::TabNext => {
                let configs = tab_configs(state.app.logged_in);
                let current_idx = configs
                    .iter()
                    .position(|c| c.view == state.app.view)
                    .unwrap_or(0);
                let next_view = configs[(current_idx + 1) % configs.len()].view;
                state.app.view = next_view;
                effects.emit_state(&state.app);
            }
            AppCommand::TabTo { index } => {
                if let Some(&cfg) = tab_configs(state.app.logged_in).get(index) {
                    state.app.view = cfg.view;
                    effects.emit_state(&state.app);
                }
            }
            cmd @ (AppCommand::LoginGenerateQr
            | AppCommand::LoginToggleCookieInput
            | AppCommand::LoginCookieInputChar { .. }
            | AppCommand::LoginCookieInputBackspace
            | AppCommand::LoginCookieSubmit) => {
                if login::handle_login_command(
                    cmd,
                    &mut state.app,
                    &mut state.req_id,
                    &mut state.request_tracker,
                    effects,
                )
                .await
                {
                    return false;
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
                    &mut state.app,
                    &mut state.req_id,
                    &mut state.request_tracker,
                    &mut state.pending_song_url,
                    effects,
                )
                .await
                {
                    return false;
                }
            }
            cmd @ (AppCommand::PlaylistsMoveUp
            | AppCommand::PlaylistsMoveDown
            | AppCommand::PlaylistsOpenSelected
            | AppCommand::PlaylistTracksMoveUp
            | AppCommand::PlaylistTracksMoveDown
            | AppCommand::PlaylistTracksPlaySelected) => {
                if playlists::handle_playlists_command(
                    cmd,
                    &mut state.app,
                    &mut state.req_id,
                    &mut state.pending_song_url,
                    &mut state.pending_playlist_detail,
                    &mut state.pending_playlist_tracks,
                    &mut state.preload_mgr,
                    effects,
                    &mut state.next_song_cache,
                )
                .await
                {
                    return false;
                }
            }
            AppCommand::Back => {
                if playlists::handle_playlists_back_command(
                    cmd,
                    &mut state.app,
                    &mut state.pending_playlist_detail,
                    &mut state.pending_playlist_tracks,
                    effects,
                )
                .await
                {
                    return false;
                }
            }
            cmd @ (AppCommand::LyricsToggleFollow
            | AppCommand::LyricsMoveUp
            | AppCommand::LyricsMoveDown
            | AppCommand::LyricsGotoCurrent
            | AppCommand::LyricsOffsetAddMs { .. }) => {
                lyrics::handle_lyrics_command(
                    cmd,
                    &mut state.app,
                    &mut state.settings,
                    data_dir,
                    effects,
                )
                .await;
            }
            cmd @ (AppCommand::PlayerTogglePause
            | AppCommand::PlayerStop
            | AppCommand::PlayerPrev
            | AppCommand::PlayerNext
            | AppCommand::PlayerSeekBackwardMs { .. }
            | AppCommand::PlayerSeekForwardMs { .. }) => {
                let mut ctx = player::control::PlayerControlCtx {
                    req_id: &mut state.req_id,
                    pending_song_url: &mut state.pending_song_url,
                    next_song_cache: &mut state.next_song_cache,
                    effects,
                };
                player::control::handle_player_control_command(cmd, &mut state.app, &mut ctx).await;
            }
            cmd @ (AppCommand::PlayerVolumeDown
            | AppCommand::PlayerVolumeUp
            | AppCommand::PlayerCycleMode) => {
                settings_handlers::handle_player_settings_command(
                    cmd,
                    &mut state.app,
                    &mut state.settings,
                    data_dir,
                    effects,
                    &mut state.next_song_cache,
                )
                .await;
            }
            cmd @ (AppCommand::SettingsMoveUp
            | AppCommand::SettingsMoveDown
            | AppCommand::SettingsDecrease
            | AppCommand::SettingsIncrease) => {
                settings_handlers::handle_settings_command(
                    cmd,
                    &mut state.app,
                    &mut state.settings,
                    data_dir,
                    effects,
                    &mut state.next_song_cache,
                )
                .await;
            }
            AppCommand::SettingsActivate => {
                match settings_handlers::handle_settings_activate_command(&mut state.app, effects)
                    .await
                {
                    Some(true) => return false, // 已处理且需 continue
                    Some(false) => {}           // 未处理（继续处理登出）
                    None => {}
                }
                // 登出处理逻辑保持不变
                if !state.app.logged_in {
                    state.app.settings_status = "未登录，无需退出".to_owned();
                    effects.emit_state(&state.app);
                    return false;
                }

                tracing::info!("用户触发：退出登录");
                effects
                    .send_audio_warn(AudioCommand::Stop, "AudioWorker 通道已关闭：Stop 发送失败");
                let id = utils::next_id(&mut state.req_id);
                effects.send_netease_hi_warn(
                    NeteaseCommand::LogoutLocal { req_id: id },
                    "NeteaseActor 通道已关闭：LogoutLocal 发送失败",
                );

                state.request_tracker.reset_all();
                state.pending_song_url = None;
                state.pending_playlists = None;
                state.pending_playlist_detail = None;
                state.pending_playlist_tracks = None;
                state.pending_lyric = None;

                state.preload_mgr.reset(&mut state.app);
                state.next_song_cache.reset();
                logout::reset_app_after_logout(&mut state.app);
                state.app.login_status = "已退出登录（已清理本地cookie），按 l 重新登录".to_owned();
                effects.emit_state(&state.app);
            }
        },
        CoreMsg::Netease(evt) => {
            if login::handle_login_event(
                &evt,
                &mut state.app,
                &mut state.req_id,
                &mut state.request_tracker,
                &mut state.pending_playlists,
                effects,
            )
            .await
            {
                return false;
            }

            match evt {
                NeteaseEvent::Playlists {
                    req_id: id,
                    playlists,
                } => {
                    if !playlists::handle_playlists_event(
                        id,
                        playlists,
                        &mut state.app,
                        &mut state.pending_playlists,
                        &mut state.preload_mgr,
                        effects,
                        &mut state.req_id,
                    )
                    .await
                    {
                        return false;
                    }
                }
                NeteaseEvent::PlaylistTrackIds {
                    req_id: id,
                    playlist_id,
                    ids,
                } => {
                    // 先检查预加载管理器是否拥有此请求
                    if state.preload_mgr.owns_req(id)
                        && state
                            .preload_mgr
                            .on_playlist_track_ids(
                                &mut state.app,
                                effects,
                                &mut state.req_id,
                                id,
                                playlist_id,
                                &ids,
                            )
                            .await
                    {
                        playlists::refresh_playlist_list_status(&mut state.app);
                        effects.emit_state(&state.app);
                        return false;
                    }

                    match playlists::handle_playlist_detail_event(
                        id,
                        playlist_id,
                        ids,
                        &mut state.app,
                        &mut state.pending_playlist_detail,
                        &mut state.pending_playlist_tracks,
                        &state.preload_mgr,
                        effects,
                        &mut state.req_id,
                    )
                    .await
                    {
                        Some(true) => return false, // 已处理且需 continue
                        Some(false) => {}           // 未处理（预加载管理器处理）
                        None => return false,       // req_id 不匹配
                    }
                }
                NeteaseEvent::Songs { req_id: id, songs } => {
                    // 先检查预加载管理器是否拥有此请求
                    if state.preload_mgr.owns_req(id)
                        && state
                            .preload_mgr
                            .on_songs(&mut state.app, effects, &mut state.req_id, id, &songs)
                            .await
                    {
                        playlists::refresh_playlist_list_status(&mut state.app);
                        effects.emit_state(&state.app);
                        return false;
                    }

                    match playlists::handle_songs_event(
                        id,
                        songs,
                        &mut state.app,
                        &mut state.pending_playlist_tracks,
                        &mut state.preload_mgr,
                        effects,
                        &mut state.req_id,
                    )
                    .await
                    {
                        Some(true) => return false, // 已处理且需 continue
                        Some(false) => {}           // 未处理
                        None => {}                  // 不应该发生
                    }
                }
                NeteaseEvent::SearchSongs { req_id: id, songs } => {
                    if !search::handle_search_songs_event(
                        id,
                        &songs,
                        &mut state.app,
                        &mut state.request_tracker,
                        effects,
                    )
                    .await
                    {
                        return false;
                    }
                }
                NeteaseEvent::SongUrl {
                    req_id: id,
                    song_url,
                } => {
                    // 检查是否为预缓存请求
                    if state.next_song_cache.owns_req(id) {
                        state
                            .next_song_cache
                            .on_song_url(id, &song_url, effects, &state.app);
                        return false;
                    }

                    if let Some((pending_id, title)) = state.pending_song_url.take() {
                        if pending_id != id {
                            return false;
                        }
                        state.app.play_status = "开始播放...".to_owned();
                        state.app.play_song_id = Some(song_url.id);
                        effects.emit_state(&state.app);
                        effects.send_audio_warn(
                            AudioCommand::PlayTrack {
                                id: song_url.id,
                                br: state.app.play_br,
                                url: song_url.url,
                                title,
                            },
                            "AudioWorker 通道已关闭：PlayTrack 发送失败",
                        );
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
                        &mut state.app,
                        &mut state.pending_lyric,
                        effects,
                    )
                    .await
                    {
                        return false;
                    }
                }
                NeteaseEvent::LoggedOut { req_id } => {
                    tracing::debug!(req_id, "NeteaseActor: LoggedOut");
                }
                NeteaseEvent::Error { req_id, message } => {
                    // 检查是否为预缓存请求
                    if state.next_song_cache.on_error(req_id) {
                        tracing::warn!(req_id, "预缓存失败: {}", message);
                        return false;
                    }

                    if state.preload_mgr.on_error(&mut state.app, req_id, &message) {
                        playlists::refresh_playlist_list_status(&mut state.app);
                        effects.emit_state(&state.app);
                        return false;
                    }

                    match state.app.view {
                        View::Login => state.app.login_status = format!("错误: {message}"),
                        View::Playlists => state.app.playlists_status = format!("错误: {message}"),
                        View::Search => state.app.search_status = format!("错误: {message}"),
                        View::Lyrics => state.app.lyrics_status = format!("错误: {message}"),
                        View::Settings => state.app.settings_status = format!("错误: {message}"),
                    }
                    effects.emit_state(&state.app);
                }
                NeteaseEvent::AnonymousReady { req_id } => {
                    tracing::debug!(req_id, "NeteaseActor: AnonymousReady");
                }
                _ => {}
            }
        }
        CoreMsg::Audio(evt) => {
            let is_stopped = matches!(evt, AudioEvent::Stopped);

            let mut ctx = player::audio::AudioEventCtx {
                pending_song_url: &mut state.pending_song_url,
                pending_lyric: &mut state.pending_lyric,
                req_id: &mut state.req_id,
                next_song_cache: &mut state.next_song_cache,
            };
            player::audio::handle_audio_event(&mut state.app, evt, &mut ctx, effects).await;

            // 播放停止时失效预缓存
            if is_stopped {
                state.next_song_cache.reset();
            }

            effects.emit_state(&state.app);
        }
    }

    false
}

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
        let mut state = CoreState {
            app: App::default(),
            req_id: 1,
            preload_mgr: PreloadManager::default(),
            next_song_cache: NextSongCacheManager::default(),
            settings: settings::load_settings(&data_dir),
            request_tracker: RequestTracker::new(),
            pending_song_url: None,
            pending_playlists: None,
            pending_playlist_detail: None,
            pending_playlist_tracks: None,
            pending_lyric: None,
        };

        settings_handlers::apply_settings_to_app(&mut state.app, &state.settings);
        let _ = tx_audio.send(AudioCommand::SetCacheBr(state.app.play_br));

        let mut qr_poll = tokio::time::interval(Duration::from_secs(2));
        let dispatch = CoreDispatch {
            tx_netease_hi: &tx_netease_hi,
            tx_netease_lo: &tx_netease_lo,
            tx_audio: &tx_audio,
            tx_evt: &tx_evt,
        };

        loop {
            let msg = tokio::select! {
                _ = qr_poll.tick() => CoreMsg::QrPoll,
                Some(cmd) = rx_cmd.recv() => CoreMsg::Ui(cmd),
                Some(evt) = rx_netease.recv() => CoreMsg::Netease(evt),
                Some(evt) = rx_audio_evt.recv() => CoreMsg::Audio(evt),
            };

            let mut effects = CoreEffects::default();
            let should_quit = reduce(msg, &mut state, &mut effects, &data_dir).await;
            run_effects(effects, &dispatch).await;
            if should_quit {
                break;
            }
        }
    });

    (tx_cmd, rx_evt)
}
