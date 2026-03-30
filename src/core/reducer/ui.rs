use super::{CoreState, UiAction};
use crate::app::{Toast, UiFocus, default_menu_items};
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
        AppCommand::ToastDismiss => {
            state.app.toast = None;
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::MenuOpen => {
            state.app.menu_visible = true;
            state.app.menu_selected = 0;
            state.app.menu_items = default_menu_items();
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::MenuCancel => {
            state.app.menu_visible = false;
            effects.emit_state(&state.app);
            UiAction::Handled
        }
        AppCommand::MenuSelect => {
            let menu_visible = state.app.menu_visible;
            let selected = state.app.menu_selected;
            let items = &state.app.menu_items;
            if menu_visible && selected < items.len() {
                let item_name = items[selected].clone();
                state.app.menu_visible = false;
                effects.set_toast(Toast::info(format!("「{}」功能即将上线", item_name)));
                effects.emit_state(&state.app);
            }
            UiAction::Handled
        }
        AppCommand::MenuMoveUp => {
            if state.app.menu_visible && state.app.menu_selected > 0 {
                state.app.menu_selected -= 1;
                effects.emit_state(&state.app);
            }
            UiAction::Handled
        }
        AppCommand::MenuMoveDown => {
            if state.app.menu_visible {
                let max_idx = state.app.menu_items.len().saturating_sub(1);
                if state.app.menu_selected < max_idx {
                    state.app.menu_selected += 1;
                    effects.emit_state(&state.app);
                }
            }
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
    use crate::app::default_menu_items;

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

    // ============================================================
    // Menu reducer tests
    // ============================================================

    /// VAL-MENU-001: MenuOpen 设置 menu_visible=true，重置 menu_selected
    #[tokio::test]
    async fn menu_open_sets_visible_and_resets_selected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        assert!(!state.app.menu_visible);

        let outcome = handle_ui(&AppCommand::MenuOpen, &mut state, &mut effects).await;
        assert!(matches!(outcome, UiAction::Handled));
        assert!(state.app.menu_visible);
        assert_eq!(state.app.menu_selected, 0);
        assert!(!state.app.menu_items.is_empty());
    }

    /// VAL-MENU-003: MenuCancel 设置 menu_visible=false
    #[tokio::test]
    async fn menu_cancel_hides_menu() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // First open the menu
        handle_ui(&AppCommand::MenuOpen, &mut state, &mut effects).await;
        assert!(state.app.menu_visible);

        // Then cancel
        let outcome = handle_ui(&AppCommand::MenuCancel, &mut state, &mut effects).await;
        assert!(matches!(outcome, UiAction::Handled));
        assert!(!state.app.menu_visible);
    }

    /// VAL-MENU-004: MenuSelect 关闭菜单并产生 toast
    #[tokio::test]
    async fn menu_select_closes_and_shows_toast() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Open menu and set selected to 1
        handle_ui(&AppCommand::MenuOpen, &mut state, &mut effects).await;
        state.app.menu_selected = 1;

        let outcome = handle_ui(&AppCommand::MenuSelect, &mut state, &mut effects).await;
        assert!(matches!(outcome, UiAction::Handled));
        assert!(!state.app.menu_visible);

        // Verify SetToast effect was produced
        let has_toast = effects
            .actions
            .iter()
            .any(|e| matches!(e, crate::core::effects::CoreEffect::SetToast(_)));
        assert!(has_toast, "应产生 SetToast 效果");
    }

    /// VAL-MENU-005: MenuMoveDown/Up 正确移动，边界安全
    #[tokio::test]
    async fn menu_move_up_down_boundary_safe() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = CoreEffects::default();

        // Open menu
        handle_ui(&AppCommand::MenuOpen, &mut state, &mut effects).await;
        let item_count = state.app.menu_items.len();
        assert!(item_count >= 4);

        // At index 0, moving up should not underflow
        state.app.menu_selected = 0;
        let mut effects2 = CoreEffects::default();
        handle_ui(&AppCommand::MenuMoveUp, &mut state, &mut effects2).await;
        assert_eq!(state.app.menu_selected, 0, "索引 0 上移应保持 0");

        // Move down to last item
        for _ in 0..item_count + 2 {
            let mut e = CoreEffects::default();
            handle_ui(&AppCommand::MenuMoveDown, &mut state, &mut e).await;
        }
        assert_eq!(
            state.app.menu_selected,
            item_count - 1,
            "不应超出最后一个索引"
        );
    }

    /// VAL-MENU-006: 菜单操作不影响 view/ui_focus 等底层状态
    #[tokio::test]
    async fn menu_operations_do_not_affect_underlying_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        state.app.view = crate::app::View::Playlists;
        state.app.ui_focus = UiFocus::BodyCenter;
        state.app.search_selected = 5;
        let mut effects = CoreEffects::default();

        // Open menu
        handle_ui(&AppCommand::MenuOpen, &mut state, &mut effects).await;
        assert_eq!(state.app.view, crate::app::View::Playlists);
        assert_eq!(state.app.ui_focus, UiFocus::BodyCenter);

        // Move in menu
        handle_ui(&AppCommand::MenuMoveDown, &mut state, &mut effects).await;
        assert_eq!(state.app.view, crate::app::View::Playlists);
        assert_eq!(state.app.ui_focus, UiFocus::BodyCenter);

        // Cancel menu
        handle_ui(&AppCommand::MenuCancel, &mut state, &mut effects).await;
        assert_eq!(state.app.view, crate::app::View::Playlists);
        assert_eq!(state.app.ui_focus, UiFocus::BodyCenter);
    }

    /// VAL-MENU-002: default_menu_items 至少 4 个
    #[test]
    fn default_menu_items_len_at_least_4() {
        let items = default_menu_items();
        assert!(items.len() >= 4, "菜单应有至少 4 个选项");
    }
}
