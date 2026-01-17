use crate::app::App;
use crate::audio_worker::{AudioBackend, AudioCommand, AudioEvent, AudioSettings};
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::NeteaseEvent;
use crate::settings as app_settings;

use std::time::Duration;
use tokio::sync::mpsc;

use crate::core::effects::{CoreDispatch, CoreEffects, run_effects};
use crate::core::infra::{NextSongCacheManager, PreloadManager, RequestKey, RequestTracker};

use crate::features::settings as settings_handlers;

mod login;
mod lyrics;
mod player;
mod playlists;
mod search;
mod settings;
mod ui;

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
    settings: app_settings::AppSettings,
    request_tracker: RequestTracker<RequestKey>,
    playlist_tracks_loader: Option<playlists::PlaylistTracksLoad>,
    song_request_titles: std::collections::HashMap<i64, String>,
}

enum UiAction {
    Handled,
    NotHandled,
    Quit,
}

impl CoreState {
    #[cfg(test)]
    fn new(data_dir: &std::path::Path) -> Self {
        Self::new_with_settings(data_dir, app_settings::load_settings(data_dir))
    }

    fn new_with_settings(_data_dir: &std::path::Path, settings: app_settings::AppSettings) -> Self {
        Self {
            app: App::default(),
            req_id: 1,
            preload_mgr: PreloadManager::default(),
            next_song_cache: NextSongCacheManager::default(),
            settings,
            request_tracker: RequestTracker::new(),
            playlist_tracks_loader: None,
            song_request_titles: Default::default(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn reduce(
    msg: CoreMsg,
    state: &mut CoreState,
    effects: &mut CoreEffects,
    data_dir: &std::path::Path,
) -> bool {
    match msg {
        CoreMsg::QrPoll => login::handle_qr_poll(state, effects),
        CoreMsg::Ui(cmd) => {
            match settings::handle_ui(&cmd, state, effects, data_dir).await {
                UiAction::Quit => return true,
                UiAction::Handled => return false,
                UiAction::NotHandled => {}
            }
            match ui::handle_ui(&cmd, state, effects).await {
                UiAction::Quit => return true,
                UiAction::Handled => return false,
                UiAction::NotHandled => {}
            }

            if matches!(
                login::handle_ui(&cmd, state, effects).await,
                UiAction::Handled
            ) {
                return false;
            }
            if matches!(
                search::handle_ui(&cmd, state, effects).await,
                UiAction::Handled
            ) {
                return false;
            }
            if matches!(
                playlists::handle_ui(&cmd, state, effects).await,
                UiAction::Handled
            ) {
                return false;
            }
            if matches!(
                player::handle_ui(&cmd, state, effects).await,
                UiAction::Handled
            ) {
                return false;
            }
            if matches!(
                lyrics::handle_ui(&cmd, state, effects, data_dir).await,
                UiAction::Handled
            ) {
                return false;
            }
        }
        CoreMsg::Netease(evt) => {
            if login::handle_netease_event(&evt, state, effects).await {
                return false;
            }
            if playlists::handle_netease_event(&evt, state, effects).await {
                return false;
            }
            if search::handle_netease_event(&evt, state, effects).await {
                return false;
            }
            if player::handle_netease_event(&evt, state, effects).await {
                return false;
            }
            if lyrics::handle_netease_event(&evt, state, effects).await {
                return false;
            }
            settings::handle_netease_event(&evt, state, effects).await;
        }
        CoreMsg::Audio(evt) => {
            player::handle_audio_event(evt, state, effects).await;
        }
    }

    false
}

pub fn spawn_app_actor(
    cfg: NeteaseClientConfig,
    audio_backend: AudioBackend,
) -> (mpsc::Sender<AppCommand>, mpsc::Receiver<AppEvent>) {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<AppCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AppEvent>(64);

    let data_dir = cfg.data_dir.clone();

    // 先加载 settings，以便创建配置化的 audio worker
    let settings = app_settings::load_settings(&data_dir);

    let (tx_netease_hi, tx_netease_lo, mut rx_netease) =
        crate::netease::actor::spawn_netease_actor(cfg);

    // Audio worker is now tokio-native, no need for std mpsc bridge
    let transfer_config = crate::audio_worker::TransferConfig {
        http_timeout_secs: settings.http_timeout_secs,
        http_connect_timeout_secs: settings.http_connect_timeout_secs,
        download_concurrency: settings.download_concurrency,
        download_retries: settings.download_retries,
        download_retry_backoff_ms: settings.download_retry_backoff_ms,
        download_retry_backoff_max_ms: settings.download_retry_backoff_max_ms,
        audio_cache_max_mb: settings.audio_cache_max_mb,
    };
    let audio_settings = AudioSettings {
        crossfade_ms: settings.crossfade_ms,
    };
    let (tx_audio, mut rx_audio_evt) = crate::audio_worker::spawn_audio_worker(
        audio_backend,
        data_dir.clone(),
        transfer_config,
        audio_settings,
    );

    tokio::spawn(async move {
        let mut state = CoreState::new_with_settings(&data_dir, settings);

        // ========== 加载保存的状态 ==========
        match crate::player_state::load_player_state(&data_dir) {
            Ok(snapshot) => {
                match crate::player_state::apply_snapshot_to_app(&snapshot, &mut state.app) {
                    Ok(()) => {
                        tracing::info!("播放状态已恢复（默认暂停）");
                    }
                    Err(e) => {
                        tracing::warn!("状态恢复失败: {}, 使用默认状态", e);
                    }
                }
            }
            Err(crate::player_state::PlayerStateError::Io(ref e))
                if e.kind() == std::io::ErrorKind::NotFound =>
            {
                tracing::debug!("首次启动，无历史状态");
            }
            Err(e) => {
                tracing::warn!("加载状态失败: {}, 使用默认状态", e);
            }
        }
        // ========== 加载完成 ==========

        settings_handlers::apply_settings_to_app(&mut state.app, &state.settings);
        let _ = tx_audio
            .send(AudioCommand::SetCacheBr(state.app.play_br))
            .await;
        let _ = tx_audio
            .send(AudioCommand::SetVolume(state.app.volume))
            .await;
        let _ = tx_audio
            .send(AudioCommand::SetCrossfadeMs(state.app.crossfade_ms))
            .await;

        let mut qr_poll = tokio::time::interval(Duration::from_secs(2));
        let mut state_save_timer = tokio::time::interval(Duration::from_secs(30));
        state_save_timer.tick().await; // 立即消耗第一个周期
        let dispatch = CoreDispatch {
            tx_netease_hi: &tx_netease_hi,
            tx_netease_lo: &tx_netease_lo,
            tx_audio: &tx_audio,
            tx_evt: &tx_evt,
        };

        loop {
            let msg = tokio::select! {
                _ = qr_poll.tick() => CoreMsg::QrPoll,
                _ = state_save_timer.tick() => {
                    // 定时保存状态
                    if let Err(e) = crate::player_state::save_player_state(&data_dir, &state.app) {
                        tracing::warn!("定时保存播放状态失败: {}", e);
                    }
                    continue; // 继续循环，不生成 CoreMsg
                }
                Some(cmd) = rx_cmd.recv() => CoreMsg::Ui(cmd),
                Some(evt) = rx_netease.recv() => CoreMsg::Netease(evt),
                Some(evt) = rx_audio_evt.recv() => CoreMsg::Audio(evt),
            };

            let mut effects = CoreEffects::default();
            let should_quit = reduce(msg, &mut state, &mut effects, &data_dir).await;
            run_effects(effects, &dispatch).await;
            if should_quit {
                // ========== 保存播放状态 ==========
                if let Err(e) = crate::player_state::save_player_state(&data_dir, &state.app) {
                    tracing::error!("保存播放状态失败: {}", e);
                } else {
                    tracing::info!("播放状态已保存");
                }
                // ========== 保存完成 ==========
                break;
            }
        }
    });

    (tx_cmd, rx_evt)
}
