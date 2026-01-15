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
