use super::{CoreState, UiAction};
use crate::core::effects::CoreEffects;
use crate::features::playlists as playlists_handlers;
use crate::messages::app::AppCommand;
use crate::netease::actor::NeteaseEvent;

pub use playlists_handlers::PlaylistTracksLoad;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> UiAction {
    match cmd {
        AppCommand::PlaylistsMoveUp
        | AppCommand::PlaylistsMoveDown
        | AppCommand::PlaylistsOpenSelected
        | AppCommand::PlaylistTracksMoveUp
        | AppCommand::PlaylistTracksMoveDown
        | AppCommand::PlaylistTracksPlaySelected => {
            let playlist_cmd = match cmd {
                AppCommand::PlaylistsMoveUp => AppCommand::PlaylistsMoveUp,
                AppCommand::PlaylistsMoveDown => AppCommand::PlaylistsMoveDown,
                AppCommand::PlaylistsOpenSelected => AppCommand::PlaylistsOpenSelected,
                AppCommand::PlaylistTracksMoveUp => AppCommand::PlaylistTracksMoveUp,
                AppCommand::PlaylistTracksMoveDown => AppCommand::PlaylistTracksMoveDown,
                AppCommand::PlaylistTracksPlaySelected => AppCommand::PlaylistTracksPlaySelected,
                _ => unreachable!("checked by outer match"),
            };
            playlists_handlers::handle_playlists_command(
                playlist_cmd,
                &mut state.app,
                &mut state.req_id,
                &mut state.request_tracker,
                &mut state.song_request_titles,
                &mut state.playlist_tracks_loader,
                &mut state.preload_mgr,
                effects,
                &mut state.next_song_cache,
            )
            .await;
            UiAction::Handled
        }
        AppCommand::Back => {
            playlists_handlers::handle_playlists_back_command(
                AppCommand::Back,
                &mut state.app,
                &mut state.playlist_tracks_loader,
                effects,
            )
            .await;
            UiAction::Handled
        }
        _ => UiAction::NotHandled,
    }
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        NeteaseEvent::Playlists { req_id, playlists } => {
            if !playlists_handlers::handle_playlists_event(
                *req_id,
                playlists.clone(),
                &mut state.app,
                &mut state.request_tracker,
                &mut state.preload_mgr,
                effects,
                &mut state.req_id,
                state.settings.preload_count,
            )
            .await
            {
                return false;
            }
            true
        }
        NeteaseEvent::PlaylistTrackIds {
            req_id,
            playlist_id,
            ids,
        } => {
            if state.preload_mgr.owns_req(*req_id)
                && state
                    .preload_mgr
                    .on_playlist_track_ids(
                        &mut state.app,
                        effects,
                        &mut state.req_id,
                        *req_id,
                        *playlist_id,
                        ids,
                    )
                    .await
            {
                playlists_handlers::refresh_playlist_list_status(&mut state.app);
                effects.emit_state(&state.app);
                return true;
            }

            match playlists_handlers::handle_playlist_detail_event(
                *req_id,
                *playlist_id,
                ids.clone(),
                &mut state.app,
                &mut state.request_tracker,
                &mut state.playlist_tracks_loader,
                &state.preload_mgr,
                effects,
                &mut state.req_id,
            )
            .await
            {
                Some(true) => true,
                Some(false) => true,
                None => false,
            }
        }
        NeteaseEvent::Songs { req_id, songs } => {
            if state.preload_mgr.owns_req(*req_id)
                && state
                    .preload_mgr
                    .on_songs(&mut state.app, effects, &mut state.req_id, *req_id, songs)
                    .await
            {
                playlists_handlers::refresh_playlist_list_status(&mut state.app);
                effects.emit_state(&state.app);
                return true;
            }

            match playlists_handlers::handle_songs_event(
                *req_id,
                songs.clone(),
                &mut state.app,
                &mut state.request_tracker,
                &mut state.playlist_tracks_loader,
                &mut state.preload_mgr,
                effects,
                &mut state.req_id,
            )
            .await
            {
                Some(true) => true,
                Some(false) => true,
                None => false,
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::handle_ui;
    use crate::app::{Playlist, PlaylistMode};
    use crate::core::effects::CoreEffect;
    use crate::core::infra::RequestKey;
    use crate::core::reducer::{CoreState, UiAction};
    use crate::messages::app::AppCommand;
    use crate::netease::actor::NeteaseCommand;

    #[tokio::test]
    async fn playlists_open_selected_requests_detail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.app.playlists = vec![Playlist {
            id: 1,
            name: "test".to_owned(),
            track_count: 0,
            special_type: 0,
        }];
        state.app.playlists_selected = 0;
        state.app.playlist_mode = PlaylistMode::List;

        let outcome = handle_ui(&AppCommand::PlaylistsOpenSelected, &mut state, &mut effects).await;

        assert!(matches!(outcome, UiAction::Handled));
        assert_eq!(state.app.playlists_status, "加载歌单歌曲中...");
        assert_eq!(
            state
                .request_tracker
                .get_pending(&RequestKey::PlaylistDetail),
            Some(1)
        );
        assert!(effects.actions.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::SendNeteaseHi {
                    cmd: NeteaseCommand::PlaylistDetail { playlist_id: 1, .. },
                    ..
                }
            )
        }));
    }
}
