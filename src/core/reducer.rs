use crate::app::App;
use crate::audio_worker::{AudioCommand, AudioEvent};
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
mod playlists;
mod player;
mod search;
mod settings;

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
    pending_song_url: Option<(u64, String)>,
    pending_playlists: Option<u64>,
    pending_playlist_detail: Option<u64>,
    pending_playlist_tracks: Option<playlists::PlaylistTracksLoad>,
    pending_lyric: Option<(u64, i64)>,
}

enum UiAction {
    Handled,
    NotHandled,
    Quit,
}

impl CoreState {
    fn new(data_dir: &std::path::Path) -> Self {
        Self {
            app: App::default(),
            req_id: 1,
            preload_mgr: PreloadManager::default(),
            next_song_cache: NextSongCacheManager::default(),
            settings: app_settings::load_settings(data_dir),
            request_tracker: RequestTracker::new(),
            pending_song_url: None,
            pending_playlists: None,
            pending_playlist_detail: None,
            pending_playlist_tracks: None,
            pending_lyric: None,
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

            if matches!(login::handle_ui(&cmd, state, effects).await, UiAction::Handled) {
                return false;
            }
            if matches!(search::handle_ui(&cmd, state, effects).await, UiAction::Handled) {
                return false;
            }
            if matches!(playlists::handle_ui(&cmd, state, effects).await, UiAction::Handled) {
                return false;
            }
            if matches!(player::handle_ui(&cmd, state, effects).await, UiAction::Handled) {
                return false;
            }
            if matches!(lyrics::handle_ui(&cmd, state, effects, data_dir).await, UiAction::Handled) {
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
) -> (mpsc::Sender<AppCommand>, mpsc::Receiver<AppEvent>) {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<AppCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AppEvent>(64);

    let data_dir = cfg.data_dir.clone();
    let (tx_netease_hi, tx_netease_lo, mut rx_netease) =
        crate::netease::actor::spawn_netease_actor(cfg);

    let (tx_audio, mut rx_audio_evt) = crate::audio_worker::spawn_audio_worker(data_dir.clone());

    tokio::spawn(async move {
        let mut state = CoreState::new(&data_dir);

        settings_handlers::apply_settings_to_app(&mut state.app, &state.settings);
        let _ = tx_audio.send(AudioCommand::SetCacheBr(state.app.play_br)).await;

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
