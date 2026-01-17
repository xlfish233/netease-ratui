use super::{CoreState, UiAction};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::core::effects::CoreEffects;
use crate::core::infra::RequestKey;
use crate::features::player;
use crate::messages::app::AppCommand;
use crate::messages::source::{Playable, SourceEvent};
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
        request_tracker: &mut state.request_tracker,
        song_request_titles: &mut state.song_request_titles,
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
        NeteaseEvent::SongUrlUnavailable { req_id, id } => {
            // 检查是否为预缓存请求
            if state.next_song_cache.owns_req(*req_id) {
                // 预缓存失败，静默忽略
                tracing::debug!(req_id, song_id = id, "预缓存歌曲无可用链接");
                return true;
            }

            // 检查 req_id 是否过期
            if !state.request_tracker.accept(&RequestKey::SongUrl, *req_id) {
                return false;
            }

            // 自动播放下一首
            tracing::info!(song_id = id, "歌曲无可用播放链接，自动跳转到下一首");
            state.app.play_status = "歌曲不可播放，自动跳过...".to_owned();

            // 清理该歌曲的请求标题（如果有）
            state.song_request_titles.remove(id);

            // 自动播放下一首
            let ctx = player::audio::AudioEventCtx {
                request_tracker: &mut state.request_tracker,
                song_request_titles: &mut state.song_request_titles,
                req_id: &mut state.req_id,
                next_song_cache: &mut state.next_song_cache,
            };
            player::playback::play_next(
                &mut state.app,
                ctx.request_tracker,
                ctx.song_request_titles,
                ctx.req_id,
                ctx.next_song_cache,
                effects,
            )
            .await;

            effects.emit_state(&state.app);
            true
        }
        NeteaseEvent::SongUrl { req_id, song_url } => {
            if state.next_song_cache.owns_req(*req_id) {
                state
                    .next_song_cache
                    .on_song_url(*req_id, song_url, effects, &state.app);
                return true;
            }

            if !state.request_tracker.accept(&RequestKey::SongUrl, *req_id) {
                return false;
            }

            if let Some(title) = state.song_request_titles.remove(&song_url.id) {
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

pub async fn handle_source_event(
    evt: &SourceEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        SourceEvent::PlayableResolved {
            req_id,
            track,
            playable,
        } => {
            if !state
                .request_tracker
                .accept(&RequestKey::SourcePlayable, *req_id)
            {
                return false;
            }

            let Some(song_id) = track.id.as_netease_song_id() else {
                state.app.play_status = "暂不支持该音源的播放".to_owned();
                effects.emit_state(&state.app);
                return true;
            };

            let Playable::RemoteUrl { url } = playable else {
                state.app.play_status = "暂不支持本地音频播放".to_owned();
                effects.emit_state(&state.app);
                return true;
            };

            if let Some(title) = state.song_request_titles.remove(&song_id) {
                state.app.play_status = "开始播放...".to_owned();
                state.app.play_song_id = Some(song_id);
                effects.emit_state(&state.app);
                effects.send_audio_warn(
                    AudioCommand::PlayTrack {
                        id: song_id,
                        br: state.app.play_br,
                        url: url.clone(),
                        title,
                    },
                    "AudioWorker 通道已关闭：PlayTrack 发送失败",
                );
            }

            true
        }
        SourceEvent::Error {
            req_id,
            track,
            message,
        } => {
            if !state
                .request_tracker
                .accept(&RequestKey::SourcePlayable, *req_id)
            {
                return false;
            }

            let song_id = track.as_ref().and_then(|t| t.id.as_netease_song_id());
            if let Some(song_id) = song_id {
                state.song_request_titles.remove(&song_id);
            }
            state.app.play_status = format!("播放失败: {message}");
            effects.emit_state(&state.app);
            true
        }
        _ => false,
    }
}

pub async fn handle_audio_event(evt: AudioEvent, state: &mut CoreState, effects: &mut CoreEffects) {
    let is_stopped = matches!(evt, AudioEvent::Stopped);

    let mut ctx = player::audio::AudioEventCtx {
        request_tracker: &mut state.request_tracker,
        song_request_titles: &mut state.song_request_titles,
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
    use crate::core::infra::RequestKey;
    use crate::core::reducer::CoreState;
    use crate::domain::ids::TrackKey;
    use crate::domain::model::SongUrl;
    use crate::messages::source::{Playable, SourceEvent};
    use crate::netease::actor::NeteaseEvent;

    #[tokio::test]
    async fn song_url_starts_playback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        let req_id = 42;
        state.request_tracker.issue(RequestKey::SongUrl, || req_id);
        state
            .song_request_titles
            .insert(7, "artist - title".to_owned());

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

    #[tokio::test]
    async fn outdated_song_url_is_dropped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.request_tracker.issue(RequestKey::SongUrl, || 1);
        state.song_request_titles.insert(1, "old".to_owned());
        state.request_tracker.issue(RequestKey::SongUrl, || 2);
        state.song_request_titles.insert(1, "new".to_owned());

        let stale = NeteaseEvent::SongUrl {
            req_id: 1,
            song_url: SongUrl {
                id: 1,
                url: "stale".to_owned(),
            },
        };
        let handled_stale = handle_netease_event(&stale, &mut state, &mut effects).await;
        assert!(!handled_stale);
        assert_eq!(state.app.play_song_id, None);
        assert_eq!(
            state.song_request_titles.get(&1).map(String::as_str),
            Some("new")
        );

        let fresh = NeteaseEvent::SongUrl {
            req_id: 2,
            song_url: SongUrl {
                id: 1,
                url: "fresh".to_owned(),
            },
        };
        let handled_fresh = handle_netease_event(&fresh, &mut state, &mut effects).await;
        assert!(handled_fresh);
        assert_eq!(state.app.play_song_id, Some(1));
        assert_eq!(state.app.play_status, "开始播放...");
        assert!(!state.song_request_titles.contains_key(&1));
    }

    #[tokio::test]
    async fn source_playable_resolved_starts_playback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        let req_id = 9;
        state.request_tracker.issue(RequestKey::SourcePlayable, || req_id);
        state
            .song_request_titles
            .insert(7, "artist - title".to_owned());

        let handled = super::handle_source_event(
            &SourceEvent::PlayableResolved {
                req_id,
                track: TrackKey::netease(7),
                playable: Playable::RemoteUrl {
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
        assert!(!state.song_request_titles.contains_key(&7));
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
