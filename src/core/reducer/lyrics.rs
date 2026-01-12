use super::{CoreState, UiAction};
use crate::core::effects::CoreEffects;
use crate::features::lyrics as lyrics_handlers;
use crate::messages::app::AppCommand;
use crate::netease::actor::NeteaseEvent;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
    data_dir: &std::path::Path,
) -> UiAction {
    let lyrics_cmd = match cmd {
        AppCommand::LyricsToggleFollow => AppCommand::LyricsToggleFollow,
        AppCommand::LyricsMoveUp => AppCommand::LyricsMoveUp,
        AppCommand::LyricsMoveDown => AppCommand::LyricsMoveDown,
        AppCommand::LyricsGotoCurrent => AppCommand::LyricsGotoCurrent,
        AppCommand::LyricsOffsetAddMs { ms } => AppCommand::LyricsOffsetAddMs { ms: *ms },
        _ => return UiAction::NotHandled,
    };

    lyrics_handlers::handle_lyrics_command(
        lyrics_cmd,
        &mut state.app,
        &mut state.settings,
        data_dir,
        effects,
    )
    .await;

    UiAction::Handled
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        NeteaseEvent::Lyric {
            req_id,
            song_id,
            lyrics,
        } => {
            lyrics_handlers::handle_lyric_event(
                *req_id,
                *song_id,
                lyrics.clone(),
                &mut state.app,
                &mut state.pending_lyric,
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
    use crate::app::View;
    use crate::core::reducer::{CoreState, UiAction};
    use crate::messages::app::AppCommand;

    #[tokio::test]
    async fn lyrics_offset_updates_settings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.app.view = View::Lyrics;
        let outcome = handle_ui(
            &AppCommand::LyricsOffsetAddMs { ms: 200 },
            &mut state,
            &mut effects,
            dir.path(),
        )
        .await;

        assert!(matches!(outcome, UiAction::Handled));
        assert_eq!(state.app.lyrics_offset_ms, 200);
        assert_eq!(state.settings.lyrics_offset_ms, 200);
    }
}
