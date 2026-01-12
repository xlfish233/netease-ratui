use super::{CoreState, UiAction};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::core::effects::CoreEffects;
use crate::features::player;
use crate::messages::app::AppCommand;
use crate::netease::actor::NeteaseEvent;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> UiAction {
    let control_cmd = match cmd {
        AppCommand::PlayerTogglePause => AppCommand::PlayerTogglePause,
        AppCommand::PlayerStop => AppCommand::PlayerStop,
        AppCommand::PlayerPrev => AppCommand::PlayerPrev,
        AppCommand::PlayerNext => AppCommand::PlayerNext,
        AppCommand::PlayerSeekBackwardMs { ms } => AppCommand::PlayerSeekBackwardMs { ms: *ms },
        AppCommand::PlayerSeekForwardMs { ms } => AppCommand::PlayerSeekForwardMs { ms: *ms },
        _ => return UiAction::NotHandled,
    };

    let mut ctx = player::control::PlayerControlCtx {
        req_id: &mut state.req_id,
        pending_song_url: &mut state.pending_song_url,
        next_song_cache: &mut state.next_song_cache,
        effects,
    };
    player::control::handle_player_control_command(control_cmd, &mut state.app, &mut ctx).await;

    UiAction::Handled
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        NeteaseEvent::SongUrl { req_id, song_url } => {
            if state.next_song_cache.owns_req(*req_id) {
                state
                    .next_song_cache
                    .on_song_url(*req_id, song_url, effects, &state.app);
                return true;
            }

            if let Some((pending_id, title)) = state.pending_song_url.take() {
                if pending_id != *req_id {
                    return false;
                }
                state.app.play_status = "开始播放...".to_owned();
                state.app.play_song_id = Some(song_url.id);
                effects.emit_state(&state.app);
                effects.send_audio_warn(
                    AudioCommand::PlayTrack {
                        id: song_url.id,
                        br: state.app.play_br,
                        url: song_url.url.clone(),
                        title,
                    },
                    "AudioWorker 通道已关闭：PlayTrack 发送失败",
                );
            }

            true
        }
        _ => false,
    }
}

pub async fn handle_audio_event(evt: AudioEvent, state: &mut CoreState, effects: &mut CoreEffects) {
    let is_stopped = matches!(evt, AudioEvent::Stopped);

    let mut ctx = player::audio::AudioEventCtx {
        pending_song_url: &mut state.pending_song_url,
        pending_lyric: &mut state.pending_lyric,
        req_id: &mut state.req_id,
        next_song_cache: &mut state.next_song_cache,
    };
    player::audio::handle_audio_event(&mut state.app, evt, &mut ctx, effects).await;

    if is_stopped {
        state.next_song_cache.reset();
    }

    effects.emit_state(&state.app);
}

#[cfg(test)]
mod tests {
    use super::handle_netease_event;
    use crate::audio_worker::AudioCommand;
    use crate::core::effects::CoreEffect;
    use crate::core::reducer::CoreState;
    use crate::domain::model::SongUrl;
    use crate::netease::actor::NeteaseEvent;

    #[tokio::test]
    async fn song_url_starts_playback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        let req_id = 42;
        state.pending_song_url = Some((req_id, "artist - title".to_owned()));

        let handled = handle_netease_event(
            &NeteaseEvent::SongUrl {
                req_id,
                song_url: SongUrl {
                    id: 7,
                    url: "http://example.com".to_owned(),
                },
            },
            &mut state,
            &mut effects,
        )
        .await;

        assert!(handled);
        assert_eq!(state.app.play_status, "开始播放...");
        assert_eq!(state.app.play_song_id, Some(7));
        assert!(effects.actions.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::SendAudio {
                    cmd: AudioCommand::PlayTrack { id: 7, .. },
                    ..
                }
            )
        }));
    }
}
