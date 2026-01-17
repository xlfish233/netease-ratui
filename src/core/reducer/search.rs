use super::{CoreState, UiAction};
use crate::core::effects::CoreEffects;
use crate::features::search as search_handlers;
use crate::messages::app::AppCommand;
use crate::messages::source::SourceEvent;
use crate::netease::actor::NeteaseEvent;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> UiAction {
    let search_cmd = match cmd {
        AppCommand::SearchSubmit => AppCommand::SearchSubmit,
        AppCommand::SearchInputBackspace => AppCommand::SearchInputBackspace,
        AppCommand::SearchInputChar { c } => AppCommand::SearchInputChar { c: *c },
        AppCommand::SearchMoveUp => AppCommand::SearchMoveUp,
        AppCommand::SearchMoveDown => AppCommand::SearchMoveDown,
        AppCommand::SearchPlaySelected => AppCommand::SearchPlaySelected,
        _ => return UiAction::NotHandled,
    };

    search_handlers::handle_search_command(
        search_cmd,
        &mut state.app,
        &mut state.req_id,
        &mut state.request_tracker,
        &mut state.song_request_titles,
        effects,
    )
    .await;

    UiAction::Handled
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    _state: &mut CoreState,
    _effects: &mut CoreEffects,
) -> bool {
    let _ = evt;
    false
}

pub async fn handle_source_event(
    evt: &SourceEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        SourceEvent::SearchTracks { req_id, tracks } => {
            search_handlers::handle_search_tracks_event(
                *req_id,
                tracks,
                &mut state.app,
                &mut state.request_tracker,
                effects,
            )
            .await
        }
        SourceEvent::Error { req_id, .. } => {
            search_handlers::handle_search_error_event(
                *req_id,
                evt,
                &mut state.app,
                &mut state.request_tracker,
                effects,
            )
            .await
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::handle_ui;
    use crate::core::effects::CoreEffect;
    use crate::core::reducer::{CoreState, UiAction};
    use crate::domain::ids::TrackKey;
    use crate::messages::app::AppCommand;
    use crate::messages::source::SourceCommand;
    use crate::messages::source::{SourceEvent, TrackSummary};

    #[tokio::test]
    async fn search_submit_emits_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.app.search_input = "hello".to_owned();
        let outcome = handle_ui(&AppCommand::SearchSubmit, &mut state, &mut effects).await;

        assert!(matches!(outcome, UiAction::Handled));
        assert_eq!(state.app.search_status, "搜索中...");
        assert!(effects.actions.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::SendSource { cmd: SourceCommand::SearchTracks { .. }, .. }
            )
        }));
    }

    #[tokio::test]
    async fn outdated_search_response_is_dropped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.app.search_input = "first".to_owned();
        let _ = handle_ui(&AppCommand::SearchSubmit, &mut state, &mut effects).await;
        state.app.search_input = "second".to_owned();
        let _ = handle_ui(&AppCommand::SearchSubmit, &mut state, &mut effects).await;

        let stale_evt = SourceEvent::SearchTracks {
            req_id: 1,
            tracks: vec![TrackSummary {
                key: TrackKey::netease(1),
                title: "old".to_owned(),
                artists: "a".to_owned(),
            }],
        };
        let handled_stale = super::handle_source_event(&stale_evt, &mut state, &mut effects).await;
        assert!(!handled_stale);
        assert!(state.app.search_results.is_empty());
        assert_eq!(state.app.search_status, "搜索中...");

        let fresh_evt = SourceEvent::SearchTracks {
            req_id: 2,
            tracks: vec![TrackSummary {
                key: TrackKey::netease(2),
                title: "new".to_owned(),
                artists: "b".to_owned(),
            }],
        };
        let handled_fresh = super::handle_source_event(&fresh_evt, &mut state, &mut effects).await;

        assert!(handled_fresh);
        assert_eq!(state.app.search_results.len(), 1);
        assert_eq!(state.app.search_results[0].id, 2);
        assert_eq!(state.app.search_status, "结果: 1 首");
    }
}
