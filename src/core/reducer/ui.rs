use super::{CoreState, UiAction};
use crate::app::UiFocus;
use crate::core::effects::CoreEffects;
use crate::messages::app::AppCommand;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> UiAction {
    match cmd {
        AppCommand::UiFocusNext => {
            state.app.ui_focus = next_focus(state.app.ui_focus);
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::UiFocusPrev => {
            state.app.ui_focus = prev_focus(state.app.ui_focus);
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::UiFocusSet { focus } => {
            state.app.ui_focus = *focus;
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::UiToggleHelp => {
            state.app.help_visible = !state.app.help_visible;
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        _ => UiAction::NotHandled,
    }
}

fn next_focus(focus: UiFocus) -> UiFocus {
    match focus {
        UiFocus::HeaderSearch => UiFocus::BodyLeft,
        UiFocus::BodyLeft => UiFocus::BodyCenter,
        UiFocus::BodyCenter => UiFocus::BodyRight,
        UiFocus::BodyRight => UiFocus::HeaderSearch,
    }
}

fn prev_focus(focus: UiFocus) -> UiFocus {
    match focus {
        UiFocus::HeaderSearch => UiFocus::BodyRight,
        UiFocus::BodyLeft => UiFocus::HeaderSearch,
        UiFocus::BodyCenter => UiFocus::BodyLeft,
        UiFocus::BodyRight => UiFocus::BodyCenter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ui_focus_set_sets_focus_correctly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Test setting focus to each possible value
        let test_cases = vec![
            UiFocus::HeaderSearch,
            UiFocus::BodyLeft,
            UiFocus::BodyCenter,
            UiFocus::BodyRight,
        ];

        for focus in test_cases {
            let cmd = AppCommand::UiFocusSet { focus };
            let outcome = handle_ui(&cmd, &mut state, &mut effects).await;

            assert!(matches!(outcome, UiAction::Handled));
            assert_eq!(state.app.ui_focus, focus);
        }
    }

    #[tokio::test]
    async fn ui_focus_next_cycles_forward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Starting focus is BodyCenter (from App::default)
        assert_eq!(state.app.ui_focus, UiFocus::BodyCenter);

        // Test the full cycle starting from BodyCenter
        let expected_sequence = vec![
            UiFocus::BodyRight,    // BodyCenter -> BodyRight
            UiFocus::HeaderSearch, // BodyRight -> HeaderSearch
            UiFocus::BodyLeft,     // HeaderSearch -> BodyLeft
            UiFocus::BodyCenter,   // BodyLeft -> BodyCenter
        ];

        for expected_focus in expected_sequence {
            let outcome = handle_ui(&AppCommand::UiFocusNext, &mut state, &mut effects).await;
            assert!(matches!(outcome, UiAction::Handled));
            assert_eq!(state.app.ui_focus, expected_focus);
        }
    }

    #[tokio::test]
    async fn ui_focus_prev_cycles_backward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Starting focus is BodyCenter (from App::default)
        assert_eq!(state.app.ui_focus, UiFocus::BodyCenter);

        // Test the full cycle in reverse starting from BodyCenter
        let expected_sequence = vec![
            UiFocus::BodyLeft,     // BodyCenter -> BodyLeft
            UiFocus::HeaderSearch, // BodyLeft -> HeaderSearch
            UiFocus::BodyRight,    // HeaderSearch -> BodyRight
            UiFocus::BodyCenter,   // BodyRight -> BodyCenter
        ];

        for expected_focus in expected_sequence {
            let outcome = handle_ui(&AppCommand::UiFocusPrev, &mut state, &mut effects).await;
            assert!(matches!(outcome, UiAction::Handled));
            assert_eq!(state.app.ui_focus, expected_focus);
        }
    }

    #[tokio::test]
    async fn ui_toggle_help_toggles_visibility() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Initially false
        assert!(!state.app.help_visible);

        // Toggle to true
        let outcome = handle_ui(&AppCommand::UiToggleHelp, &mut state, &mut effects).await;
        assert!(matches!(outcome, UiAction::Handled));
        assert!(state.app.help_visible);

        // Toggle back to false
        let outcome = handle_ui(&AppCommand::UiToggleHelp, &mut state, &mut effects).await;
        assert!(matches!(outcome, UiAction::Handled));
        assert!(!state.app.help_visible);
    }
}
