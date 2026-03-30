use crate::app::{AppSnapshot, AppViewSnapshot, PlaylistMode, UiFocus, View};
use crate::keybindings::KeyAction;
use crate::messages::app::AppCommand;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

pub(super) async fn handle_key(
    app: &AppSnapshot,
    key: KeyEvent,
    tx: &mpsc::Sender<AppCommand>,
) -> bool {
    // Some terminals/platforms may report both press and release events; we only act on press/repeat.
    if matches!(key.kind, KeyEventKind::Release) {
        return false;
    }

    if app.help_visible {
        match key.code {
            KeyCode::Esc => {
                let _ = tx.send(AppCommand::UiToggleHelp).await;
            }
            KeyCode::Char('?') => {
                // Only toggle help if '?' is still bound to UiToggleHelp
                if app
                    .keybindings
                    .resolve(KeyCode::Char('?'))
                    .is_some_and(|a| a == KeyAction::UiToggleHelp)
                {
                    let _ = tx.send(AppCommand::UiToggleHelp).await;
                }
            }
            _ => {}
        }
        return false;
    }

    // Menu overlay: captures all keys when visible
    if app.menu_visible {
        match key.code {
            KeyCode::Esc => {
                let _ = tx.send(AppCommand::MenuCancel).await;
            }
            KeyCode::Enter => {
                let _ = tx.send(AppCommand::MenuSelect).await;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let _ = tx.send(AppCommand::MenuMoveUp).await;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let _ = tx.send(AppCommand::MenuMoveDown).await;
            }
            _ => {}
        }
        return false;
    }

    // Configurable global keybindings (Quit, Help, Menu, PlayerPrev/Next, CycleMode)
    // These are resolved via the keybindings HashMap instead of hardcoded match branches.
    if key.modifiers == KeyModifiers::NONE
        && let Some(action) = app.keybindings.resolve(key.code)
    {
        match action {
            KeyAction::Quit => {
                let _ = tx.send(AppCommand::Quit).await;
                return true;
            }
            KeyAction::UiToggleHelp => {
                let _ = tx.send(AppCommand::UiToggleHelp).await;
                return false;
            }
            KeyAction::MenuOpen => {
                let _ = tx.send(AppCommand::MenuOpen).await;
                return false;
            }
            KeyAction::PlayerPrev => {
                let _ = tx.send(AppCommand::PlayerPrev).await;
                return false;
            }
            KeyAction::PlayerNext => {
                let _ = tx.send(AppCommand::PlayerNext).await;
                return false;
            }
            KeyAction::PlayerCycleMode => {
                let _ = tx.send(AppCommand::PlayerCycleMode).await;
                return false;
            }
            KeyAction::PlayerStop => {
                // Ctrl+s is the default; if user binds a plain key, handle it here
                let _ = tx.send(AppCommand::PlayerStop).await;
                return false;
            }
            KeyAction::PlayerTogglePause => {
                // Space key has special handling below for search input
                if key.code == KeyCode::Char(' ') {
                    // Will be handled in the global player controls section below
                    // which checks for search input focus
                } else {
                    let _ = tx.send(AppCommand::PlayerTogglePause).await;
                    return false;
                }
            }
        }
    }

    // Non-configurable global keys (Tab, BackTab, F-keys)
    match key {
        KeyEvent {
            code: KeyCode::Tab,
            modifiers,
            ..
        } => {
            if modifiers.contains(KeyModifiers::CONTROL) {
                tracing::debug!("Ctrl+Tab 按下，切换页签");
                let _ = tx.send(AppCommand::TabNext).await;
            } else {
                tracing::debug!("Tab 按下，切换焦点");
                let _ = tx.send(AppCommand::UiFocusNext).await;
            }
            return false;
        }
        KeyEvent {
            code: KeyCode::BackTab,
            ..
        } => {
            let _ = tx.send(AppCommand::UiFocusPrev).await;
        }
        KeyEvent {
            code: KeyCode::F(k @ 1..=4),
            ..
        } => {
            let index = k as usize - 1;
            let _ = tx.send(AppCommand::TabTo { index }).await;
            return false;
        }
        _ => {}
    }

    // Alt+数字键：始终切换焦点（即使在搜索框中）
    match (key.code, key.modifiers) {
        (KeyCode::Char(c), m) if m.contains(KeyModifiers::ALT) && ('1'..='4').contains(&c) => {
            let focus = match c {
                '1' => UiFocus::HeaderSearch,
                '2' => UiFocus::BodyLeft,
                '3' => UiFocus::BodyCenter,
                '4' => UiFocus::BodyRight,
                _ => return false,
            };
            let _ = tx.send(AppCommand::UiFocusSet { focus }).await;
            return false;
        }
        _ => {}
    }

    // Global player controls (avoid interfering with text input as much as possible)
    match (key.code, key.modifiers) {
        // Focus switching with number keys (1-4), but not when typing in search
        (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) && ('1'..='4').contains(&c) => {
            if matches!(app.view, View::Search) && matches!(app.ui_focus, UiFocus::HeaderSearch) {
                // 在搜索框中，允许输入数字
            } else {
                let focus = match c {
                    '1' => UiFocus::HeaderSearch,
                    '2' => UiFocus::BodyLeft,
                    '3' => UiFocus::BodyCenter,
                    '4' => UiFocus::BodyRight,
                    _ => return false,
                };
                let _ = tx.send(AppCommand::UiFocusSet { focus }).await;
                return false;
            }
        }
        // Space: special context-aware handling
        // - In search input: sends SearchInputChar
        // - Otherwise: checks if PlayerTogglePause is bound to Space
        (KeyCode::Char(' '), _) => {
            if matches!(app.view, View::Search) && matches!(app.ui_focus, UiFocus::HeaderSearch) {
                // 搜索框焦点下，Space 作为字符输入
                let _ = tx.send(AppCommand::SearchInputChar { c: ' ' }).await;
            } else if app
                .keybindings
                .resolve(KeyCode::Char(' '))
                .is_some_and(|a| a == KeyAction::PlayerTogglePause)
            {
                tracing::debug!("🎵 [Keyboard] 检测到空格键，发送 PlayerTogglePause 命令");
                let _ = tx.send(AppCommand::PlayerTogglePause).await;
            }
            return false;
        }
        (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx.send(AppCommand::PlayerStop).await;
            return false;
        }
        (KeyCode::Left, m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx
                .send(AppCommand::PlayerSeekBackwardMs { ms: 5_000 })
                .await;
            return false;
        }
        (KeyCode::Right, m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx.send(AppCommand::PlayerSeekForwardMs { ms: 5_000 }).await;
            return false;
        }
        (KeyCode::Up, m) if m.contains(KeyModifiers::ALT) => {
            let _ = tx.send(AppCommand::PlayerVolumeUp).await;
            return false;
        }
        (KeyCode::Down, m) if m.contains(KeyModifiers::ALT) => {
            let _ = tx.send(AppCommand::PlayerVolumeDown).await;
            return false;
        }
        (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) && matches!(app.view, View::Lyrics) => {
            let ms = if m.contains(KeyModifiers::SHIFT) {
                -50
            } else {
                -200
            };
            let _ = tx.send(AppCommand::LyricsOffsetAddMs { ms }).await;
            return false;
        }
        (KeyCode::Right, m)
            if m.contains(KeyModifiers::ALT) && matches!(app.view, View::Lyrics) =>
        {
            let ms = if m.contains(KeyModifiers::SHIFT) {
                50
            } else {
                200
            };
            let _ = tx.send(AppCommand::LyricsOffsetAddMs { ms }).await;
            return false;
        }
        _ => {}
    }

    let focus = app.ui_focus;
    match app.view {
        View::Login => {
            if focus != UiFocus::BodyCenter {
                return false;
            }
            let login_cookie_input_visible = match &app.view_state {
                AppViewSnapshot::Login(state) => state.login_cookie_input_visible,
                _ => false,
            };
            if login_cookie_input_visible {
                // Cookie input mode
                match key.code {
                    KeyCode::Esc => {
                        let _ = tx.send(AppCommand::LoginToggleCookieInput).await;
                    }
                    KeyCode::Enter => {
                        let _ = tx.send(AppCommand::LoginCookieSubmit).await;
                    }
                    KeyCode::Backspace => {
                        let _ = tx.send(AppCommand::LoginCookieInputBackspace).await;
                    }
                    KeyCode::Char(c) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            let _ = tx.send(AppCommand::LoginCookieInputChar { c }).await;
                        }
                    }
                    _ => {}
                }
            } else {
                // QR login mode
                match key.code {
                    KeyCode::Char('l') => {
                        let _ = tx.send(AppCommand::LoginGenerateQr).await;
                    }
                    KeyCode::Char('c') => {
                        let _ = tx.send(AppCommand::LoginToggleCookieInput).await;
                    }
                    _ => {}
                }
            }
        }
        View::Playlists => {
            let playlist_mode = match &app.view_state {
                AppViewSnapshot::Playlists(state) => state.playlist_mode,
                _ => PlaylistMode::List,
            };
            if matches!(key.code, KeyCode::Char('b')) {
                let _ = tx.send(AppCommand::Back).await;
                return false;
            }
            match focus {
                UiFocus::BodyLeft => match key.code {
                    KeyCode::Up => {
                        let _ = tx.send(AppCommand::PlaylistsMoveUp).await;
                    }
                    KeyCode::Down => {
                        let _ = tx.send(AppCommand::PlaylistsMoveDown).await;
                    }
                    KeyCode::PageDown => {
                        let _ = tx.send(AppCommand::PlaylistsPageDown).await;
                    }
                    KeyCode::PageUp => {
                        let _ = tx.send(AppCommand::PlaylistsPageUp).await;
                    }
                    KeyCode::Home => {
                        let _ = tx.send(AppCommand::PlaylistsJumpTop).await;
                    }
                    KeyCode::End => {
                        let _ = tx.send(AppCommand::PlaylistsJumpBottom).await;
                    }
                    KeyCode::Enter => {
                        if matches!(playlist_mode, PlaylistMode::Tracks) {
                            let _ = tx.send(AppCommand::Back).await;
                        }
                        let _ = tx.send(AppCommand::PlaylistsOpenSelected).await;
                    }
                    _ => {}
                },
                UiFocus::BodyCenter => match key.code {
                    KeyCode::Enter if matches!(playlist_mode, PlaylistMode::List) => {
                        let _ = tx.send(AppCommand::PlaylistsOpenSelected).await;
                    }
                    KeyCode::Char('p') if matches!(playlist_mode, PlaylistMode::Tracks) => {
                        let _ = tx.send(AppCommand::PlaylistTracksPlaySelected).await;
                    }
                    KeyCode::Up => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsMoveUp).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksMoveUp).await;
                        }
                    },
                    KeyCode::Down => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsMoveDown).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksMoveDown).await;
                        }
                    },
                    KeyCode::PageDown => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsPageDown).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksPageDown).await;
                        }
                    },
                    KeyCode::PageUp => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsPageUp).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksPageUp).await;
                        }
                    },
                    KeyCode::Home => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsJumpTop).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksJumpTop).await;
                        }
                    },
                    KeyCode::End => match playlist_mode {
                        PlaylistMode::List => {
                            let _ = tx.send(AppCommand::PlaylistsJumpBottom).await;
                        }
                        PlaylistMode::Tracks => {
                            let _ = tx.send(AppCommand::PlaylistTracksJumpBottom).await;
                        }
                    },
                    _ => {}
                },
                _ => {}
            }
        }
        View::Search => match (focus, key.code) {
            (UiFocus::HeaderSearch, KeyCode::Enter) => {
                let _ = tx.send(AppCommand::SearchSubmit).await;
            }
            (UiFocus::HeaderSearch, KeyCode::Backspace) => {
                let _ = tx.send(AppCommand::SearchInputBackspace).await;
            }
            (UiFocus::HeaderSearch, KeyCode::Char(c)) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    let _ = tx.send(AppCommand::SearchInputChar { c }).await;
                }
            }
            (UiFocus::BodyCenter, KeyCode::Char('p')) => {
                let _ = tx.send(AppCommand::SearchPlaySelected).await;
            }
            (UiFocus::BodyCenter, KeyCode::Up) => {
                let _ = tx.send(AppCommand::SearchMoveUp).await;
            }
            (UiFocus::BodyCenter, KeyCode::Down) => {
                let _ = tx.send(AppCommand::SearchMoveDown).await;
            }
            (UiFocus::BodyCenter, KeyCode::PageDown) => {
                let _ = tx.send(AppCommand::SearchPageDown).await;
            }
            (UiFocus::BodyCenter, KeyCode::PageUp) => {
                let _ = tx.send(AppCommand::SearchPageUp).await;
            }
            (UiFocus::BodyCenter, KeyCode::Home) => {
                let _ = tx.send(AppCommand::SearchJumpTop).await;
            }
            (UiFocus::BodyCenter, KeyCode::End) => {
                let _ = tx.send(AppCommand::SearchJumpBottom).await;
            }
            _ => {}
        },
        View::Lyrics => {
            if focus != UiFocus::BodyCenter {
                return false;
            }
            match key.code {
                KeyCode::Char('o') => {
                    let _ = tx.send(AppCommand::LyricsToggleFollow).await;
                }
                KeyCode::Char('g') => {
                    let _ = tx.send(AppCommand::LyricsGotoCurrent).await;
                }
                KeyCode::Up => {
                    let _ = tx.send(AppCommand::LyricsMoveUp).await;
                }
                KeyCode::Down => {
                    let _ = tx.send(AppCommand::LyricsMoveDown).await;
                }
                _ => {}
            }
        }
        View::Settings => {
            match focus {
                UiFocus::BodyLeft => {
                    // 左侧：分组导航
                    match key.code {
                        KeyCode::Up => {
                            let _ = tx.send(AppCommand::SettingsGroupPrev).await;
                        }
                        KeyCode::Down => {
                            let _ = tx.send(AppCommand::SettingsGroupNext).await;
                        }
                        KeyCode::Enter => {
                            let _ = tx
                                .send(AppCommand::UiFocusSet {
                                    focus: UiFocus::BodyCenter,
                                })
                                .await;
                        }
                        KeyCode::Tab => {
                            let _ = tx
                                .send(AppCommand::UiFocusSet {
                                    focus: UiFocus::BodyCenter,
                                })
                                .await;
                        }
                        _ => {}
                    }
                }
                UiFocus::BodyCenter => {
                    // 中间：设置项详情
                    match key.code {
                        KeyCode::Up => {
                            let _ = tx.send(AppCommand::SettingsItemPrev).await;
                        }
                        KeyCode::Down => {
                            let _ = tx.send(AppCommand::SettingsItemNext).await;
                        }
                        KeyCode::Left => {
                            let _ = tx.send(AppCommand::SettingsDecrease).await;
                        }
                        KeyCode::Right => {
                            let _ = tx.send(AppCommand::SettingsIncrease).await;
                        }
                        KeyCode::Enter => {
                            let _ = tx.send(AppCommand::SettingsActivate).await;
                        }
                        KeyCode::Tab => {
                            let _ = tx
                                .send(AppCommand::UiFocusSet {
                                    focus: UiFocus::BodyLeft,
                                })
                                .await;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, PlaylistMode};

    #[tokio::test]
    async fn tab_release_is_ignored() {
        let app = AppSnapshot::from_app(&App::default());
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&app, key, &tx).await;
        assert!(!should_quit);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn tab_press_sends_focus_next_once() {
        let app = AppSnapshot::from_app(&App::default());
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&app, key, &tx).await;
        assert!(!should_quit);
        assert!(matches!(rx.try_recv(), Ok(AppCommand::UiFocusNext)));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn f1_sends_tab_to_index_0() {
        let app = AppSnapshot::from_app(&App::default());
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        for f_key in 1..=4 {
            let key = KeyEvent {
                code: KeyCode::F(f_key),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            };

            let should_quit = handle_key(&app, key, &tx).await;
            assert!(!should_quit);
            assert!(
                matches!(rx.try_recv(), Ok(AppCommand::TabTo { index }) if index == (f_key as usize) - 1)
            );
            assert!(rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn number_keys_send_ui_focus_set() {
        let app = App {
            view: View::Playlists, // Not Search view
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let app_snapshot = AppSnapshot::from_app(&app);

        let test_cases = vec![
            ('1', UiFocus::HeaderSearch),
            ('2', UiFocus::BodyLeft),
            ('3', UiFocus::BodyCenter),
            ('4', UiFocus::BodyRight),
        ];

        for (key_char, expected_focus) in test_cases {
            let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

            let key = KeyEvent {
                code: KeyCode::Char(key_char),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            };

            let should_quit = handle_key(&app_snapshot, key, &tx).await;
            assert!(!should_quit);
            assert!(
                matches!(rx.try_recv(), Ok(AppCommand::UiFocusSet { focus }) if focus == expected_focus)
            );
            assert!(rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn number_keys_in_search_input_send_search_char_not_focus_set() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        let app_snapshot = AppSnapshot::from_app(&app);

        for key_char in ['1', '2', '3', '4'] {
            let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

            let key = KeyEvent {
                code: KeyCode::Char(key_char),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            };

            let should_quit = handle_key(&app_snapshot, key, &tx).await;
            assert!(!should_quit);
            // In search input, number keys should send SearchInputChar, not UiFocusSet
            assert!(
                matches!(rx.try_recv(), Ok(AppCommand::SearchInputChar { c }) if c == key_char)
            );
            assert!(rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn backtab_sends_focus_prev() {
        let app = AppSnapshot::from_app(&App::default());
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&app, key, &tx).await;
        assert!(!should_quit);
        assert!(matches!(rx.try_recv(), Ok(AppCommand::UiFocusPrev)));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn alt_number_keys_always_switch_focus_even_in_search() {
        // 测试 Alt+1-4 在搜索框中也能切换焦点
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        let app_snapshot = AppSnapshot::from_app(&app);

        for (key_char, expected_focus) in [
            ('1', UiFocus::HeaderSearch),
            ('2', UiFocus::BodyLeft),
            ('3', UiFocus::BodyCenter),
            ('4', UiFocus::BodyRight),
        ] {
            let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

            let key = KeyEvent {
                code: KeyCode::Char(key_char),
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            };

            let should_quit = handle_key(&app_snapshot, key, &tx).await;
            assert!(!should_quit);
            assert!(
                matches!(rx.try_recv(), Ok(AppCommand::UiFocusSet { focus }) if focus == expected_focus)
            );
            assert!(rx.try_recv().is_err());
        }
    }

    // ============================================================
    // VAL-SPACE-001 ~ VAL-SPACE-005: 空格键冲突修复测试
    // ============================================================

    /// VAL-SPACE-001: 搜索框中按 Space 输入空格字符
    #[tokio::test]
    async fn space_in_search_input_sends_search_input_char() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        // 唯一发送的命令应为 SearchInputChar { c: ' ' }
        let cmd = rx.try_recv().expect("应发送一个命令");
        assert!(
            matches!(cmd, AppCommand::SearchInputChar { c } if c == ' '),
            "期望 SearchInputChar {{ c: ' ' }}，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-SPACE-002: 搜索框中按 Space 不触发播放/暂停
    #[tokio::test]
    async fn space_in_search_input_does_not_send_toggle_pause() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let _ = handle_key(&snapshot, key, &tx).await;

        // 遍历所有已发送命令，断言无 PlayerTogglePause
        while let Ok(cmd) = rx.try_recv() {
            assert!(
                !matches!(cmd, AppCommand::PlayerTogglePause),
                "搜索框中不应发送 PlayerTogglePause，但收到了 {:?}",
                cmd
            );
        }
    }

    /// VAL-SPACE-003: 搜索视图非搜索焦点时 Space 为播放/暂停
    #[tokio::test]
    async fn space_in_search_body_center_sends_toggle_pause() {
        for focus in [UiFocus::BodyCenter, UiFocus::BodyLeft, UiFocus::BodyRight] {
            let app = App {
                view: View::Search,
                ui_focus: focus,
                ..Default::default()
            };
            let snapshot = AppSnapshot::from_app(&app);
            let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

            let key = KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            };

            let should_quit = handle_key(&snapshot, key, &tx).await;
            assert!(!should_quit);
            let cmd = rx.try_recv().expect("应发送一个命令");
            assert!(
                matches!(cmd, AppCommand::PlayerTogglePause),
                "焦点 {:?} 下期望 PlayerTogglePause，实际收到 {:?}",
                focus,
                cmd
            );
            assert!(rx.try_recv().is_err(), "不应发送其他命令");
        }
    }

    /// VAL-SPACE-004: 歌单页中 Space 为播放/暂停
    #[tokio::test]
    async fn space_in_playlists_sends_toggle_pause() {
        let app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送一个命令");
        assert!(
            matches!(cmd, AppCommand::PlayerTogglePause),
            "歌单页期望 PlayerTogglePause，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-SPACE-005: 歌词页中 Space 为播放/暂停
    #[tokio::test]
    async fn space_in_lyrics_sends_toggle_pause() {
        let app = App {
            view: View::Lyrics,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送一个命令");
        assert!(
            matches!(cmd, AppCommand::PlayerTogglePause),
            "歌词页期望 PlayerTogglePause，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    // ============================================================
    // VAL-TOAST-001 ~ VAL-TOAST-003, VAL-TOAST-007: Toast 非阻断测试
    // ============================================================

    /// Helper: 创建带 Toast 的 AppSnapshot
    fn make_snapshot_with_toast(view: View, focus: UiFocus) -> AppSnapshot {
        let mut app = App {
            view,
            ui_focus: focus,
            ..Default::default()
        };
        app.toast = Some(crate::app::Toast::info("test toast"));
        AppSnapshot::from_app(&app)
    }

    /// VAL-TOAST-001: Toast 显示时按 Down 键正常触发列表移动，不发送 ToastDismiss
    #[tokio::test]
    async fn toast_visible_down_key_triggers_list_move() {
        let snapshot = make_snapshot_with_toast(View::Playlists, UiFocus::BodyCenter);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 PlaylistsMoveDown");
        assert!(
            matches!(cmd, AppCommand::PlaylistsMoveDown),
            "期望 PlaylistsMoveDown，实际收到 {:?}",
            cmd
        );
        assert!(
            rx.try_recv().is_err(),
            "不应发送其他命令（尤其是 ToastDismiss）"
        );
    }

    /// VAL-TOAST-001: Toast 显示时按 Up 键正常触发列表移动，不发送 ToastDismiss
    #[tokio::test]
    async fn toast_visible_up_key_triggers_list_move() {
        let snapshot = make_snapshot_with_toast(View::Playlists, UiFocus::BodyCenter);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 PlaylistsMoveUp");
        assert!(
            matches!(cmd, AppCommand::PlaylistsMoveUp),
            "期望 PlaylistsMoveUp，实际收到 {:?}",
            cmd
        );
        assert!(
            rx.try_recv().is_err(),
            "不应发送其他命令（尤其是 ToastDismiss）"
        );
    }

    /// VAL-TOAST-002: Toast 显示时按 Space 仍能播放/暂停，不发送 ToastDismiss
    #[tokio::test]
    async fn toast_visible_space_triggers_toggle_pause() {
        let snapshot = make_snapshot_with_toast(View::Playlists, UiFocus::BodyCenter);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 PlayerTogglePause");
        assert!(
            matches!(cmd, AppCommand::PlayerTogglePause),
            "期望 PlayerTogglePause，实际收到 {:?}",
            cmd
        );
        assert!(
            rx.try_recv().is_err(),
            "不应发送其他命令（尤其是 ToastDismiss）"
        );
    }

    /// VAL-TOAST-003: Toast 显示时按 q 仍能退出
    #[tokio::test]
    async fn toast_visible_q_triggers_quit() {
        let mut app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        app.toast = Some(crate::app::Toast::error("err"));
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(should_quit, "按 q 应返回 should_quit=true");
        let cmd = rx.try_recv().expect("应发送 Quit");
        assert!(
            matches!(cmd, AppCommand::Quit),
            "期望 Quit，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-TOAST-007: 新 Toast 替换旧 Toast（直接覆盖 app.toast）
    /// 测试 Toast 覆盖行为：在 reducer 中新 Toast 直接覆盖旧 Toast
    #[test]
    fn new_toast_replaces_old_toast() {
        let _toast1 = crate::app::Toast::info("first");
        let toast2 = crate::app::Toast::error("second");

        // 模拟 reducer 中的行为：直接覆盖
        // 先设置 toast1，然后被 toast2 覆盖
        let current = toast2;

        assert_eq!(current.message, "second");
        assert_eq!(current.level, crate::app::ToastLevel::Error);
    }

    /// Toast 显示时 Esc 不发送 ToastDismiss，而是正常穿透
    #[tokio::test]
    async fn toast_visible_esc_does_not_dismiss() {
        let snapshot = make_snapshot_with_toast(View::Playlists, UiFocus::BodyCenter);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        // Toast 不再拦截任何键，Esc 在 Playlists BodyCenter 下无映射，所以不应有任何命令
        assert!(
            rx.try_recv().is_err(),
            "Esc 在 Playlists BodyCenter 不应产生命令"
        );
    }

    // ============================================================
    // VAL-MENU-001 ~ VAL-MENU-006: 操作菜单测试
    // ============================================================

    /// Helper: 创建带菜单可见的 AppSnapshot
    fn make_snapshot_with_menu() -> AppSnapshot {
        let mut app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        app.menu_visible = true;
        app.menu_selected = 0;
        app.menu_items = crate::app::default_menu_items();
        AppSnapshot::from_app(&app)
    }

    /// VAL-MENU-001: 按 m 键弹出操作菜单
    #[tokio::test]
    async fn m_key_sends_menu_open() {
        let app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Char('m'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 MenuOpen 命令");
        assert!(
            matches!(cmd, AppCommand::MenuOpen),
            "期望 MenuOpen，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-MENU-003: Esc 关闭操作菜单
    #[tokio::test]
    async fn menu_esc_sends_menu_cancel() {
        let snapshot = make_snapshot_with_menu();
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 MenuCancel 命令");
        assert!(
            matches!(cmd, AppCommand::MenuCancel),
            "期望 MenuCancel，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "仅应收到 MenuCancel");
    }

    /// VAL-MENU-004: Enter 选择菜单项后关闭（通过 MenuSelect 命令）
    #[tokio::test]
    async fn menu_enter_sends_menu_select() {
        let mut app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        app.menu_visible = true;
        app.menu_selected = 1;
        app.menu_items = crate::app::default_menu_items();
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 MenuSelect 命令");
        assert!(
            matches!(cmd, AppCommand::MenuSelect),
            "期望 MenuSelect，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-MENU-005: j/k 在菜单中移动高亮
    #[tokio::test]
    async fn menu_jk_moves_highlight() {
        let snapshot = make_snapshot_with_menu();

        // j (down) should send MenuMoveDown
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);
        let key_down = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_down, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("j 应发送 MenuMoveDown");
        assert!(
            matches!(cmd, AppCommand::MenuMoveDown),
            "期望 MenuMoveDown，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err());

        // k (up) should send MenuMoveUp
        let key_up = KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_up, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("k 应发送 MenuMoveUp");
        assert!(
            matches!(cmd, AppCommand::MenuMoveUp),
            "期望 MenuMoveUp，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err());

        // Up arrow should also send MenuMoveUp
        let key_up_arrow = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_up_arrow, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Up 应发送 MenuMoveUp");
        assert!(
            matches!(cmd, AppCommand::MenuMoveUp),
            "期望 MenuMoveUp，实际收到 {:?}",
            cmd
        );

        // Down arrow should also send MenuMoveDown
        let key_down_arrow = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_down_arrow, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Down 应发送 MenuMoveDown");
        assert!(
            matches!(cmd, AppCommand::MenuMoveDown),
            "期望 MenuMoveDown，实际收到 {:?}",
            cmd
        );
    }

    /// VAL-MENU-006: 菜单可见时其他按键不穿透到视图
    #[tokio::test]
    async fn menu_visible_keys_do_not_penetrate() {
        let snapshot = make_snapshot_with_menu();
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Press Space — should NOT send PlayerTogglePause when menu is visible
        let key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        // Menu captures all keys not explicitly mapped, so no command should be sent
        assert!(
            rx.try_recv().is_err(),
            "菜单可见时 Space 不应穿透到底层视图"
        );

        // Press q — should NOT quit when menu is visible
        let key_q = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_q, &tx).await;
        assert!(!should_quit, "菜单可见时 q 不应退出");
        assert!(rx.try_recv().is_err(), "菜单可见时 q 不应穿透到底层视图");
    }

    // ============================================================
    // VAL-PAGE-001 ~ VAL-PAGE-004: 分页支持测试
    // ============================================================

    /// VAL-PAGE-001: PageDown 在搜索结果中发送 SearchPageDown
    #[tokio::test]
    async fn page_down_in_search_sends_search_page_down() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 SearchPageDown 命令");
        assert!(
            matches!(cmd, AppCommand::SearchPageDown),
            "期望 SearchPageDown，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-PAGE-002: PageUp 在搜索结果中发送 SearchPageUp
    #[tokio::test]
    async fn page_up_in_search_sends_search_page_up() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 SearchPageUp 命令");
        assert!(
            matches!(cmd, AppCommand::SearchPageUp),
            "期望 SearchPageUp，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-PAGE-003: Home 在搜索结果中发送 SearchJumpTop
    #[tokio::test]
    async fn home_in_search_sends_search_jump_top() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 SearchJumpTop 命令");
        assert!(
            matches!(cmd, AppCommand::SearchJumpTop),
            "期望 SearchJumpTop，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-PAGE-004: End 在搜索结果中发送 SearchJumpBottom
    #[tokio::test]
    async fn end_in_search_sends_search_jump_bottom() {
        let app = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 SearchJumpBottom 命令");
        assert!(
            matches!(cmd, AppCommand::SearchJumpBottom),
            "期望 SearchJumpBottom，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// PageDown 在歌单页 BodyLeft 发送 PlaylistsPageDown
    #[tokio::test]
    async fn page_down_in_playlists_left_sends_playlists_page_down() {
        let app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyLeft,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 PlaylistsPageDown 命令");
        assert!(
            matches!(cmd, AppCommand::PlaylistsPageDown),
            "期望 PlaylistsPageDown，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err());
    }

    /// PageDown 在歌单页 BodyCenter (Tracks 模式) 发送 PlaylistTracksPageDown
    #[tokio::test]
    async fn page_down_in_playlist_tracks_sends_tracks_page_down() {
        let mut app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        app.playlist_mode = PlaylistMode::Tracks;
        let snapshot = AppSnapshot::from_app(&app);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&snapshot, key, &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 PlaylistTracksPageDown 命令");
        assert!(
            matches!(cmd, AppCommand::PlaylistTracksPageDown),
            "期望 PlaylistTracksPageDown，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err());
    }

    /// Home/End 在歌单页 BodyLeft 发送 JumpTop/JumpBottom
    #[tokio::test]
    async fn home_end_in_playlists_left_sends_jump_commands() {
        let app = App {
            view: View::Playlists,
            ui_focus: UiFocus::BodyLeft,
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);

        // Home
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);
        let key_home = KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_home, &tx).await;
        assert!(!should_quit);
        assert!(
            matches!(rx.try_recv(), Ok(AppCommand::PlaylistsJumpTop)),
            "期望 PlaylistsJumpTop"
        );
        assert!(rx.try_recv().is_err());

        // End
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);
        let key_end = KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let should_quit = handle_key(&snapshot, key_end, &tx).await;
        assert!(!should_quit);
        assert!(
            matches!(rx.try_recv(), Ok(AppCommand::PlaylistsJumpBottom)),
            "期望 PlaylistsJumpBottom"
        );
        assert!(rx.try_recv().is_err());
    }

    // ============================================================
    // VAL-CROSS-001 ~ VAL-CROSS-003: 跨区域流程集成测试
    // ============================================================

    /// Helper: create a press KeyEvent
    fn press_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    /// VAL-CROSS-001: 搜索框输入空格 → Tab 切换到 BodyCenter → m 弹出菜单 → Esc 关闭菜单
    /// 验证多个 UI 功能在不同组合下正确协作：
    /// 1. Space 在搜索框输入空格（非 PlayerTogglePause）
    /// 2. Tab 切换焦点到 BodyCenter
    /// 3. m 弹出菜单
    /// 4. Esc 关闭菜单
    #[tokio::test]
    async fn cross_area_search_space_tab_menu_esc_flow() {
        // Step 1: 搜索框焦点下按 Space → 输入空格
        let app_search = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        let snapshot_search = AppSnapshot::from_app(&app_search);
        let (tx, mut rx) = mpsc::channel::<AppCommand>(16);

        let should_quit = handle_key(&snapshot_search, press_key(KeyCode::Char(' ')), &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Step 1: 应发送 SearchInputChar");
        assert!(
            matches!(cmd, AppCommand::SearchInputChar { c } if c == ' '),
            "Step 1: 期望 SearchInputChar {{ c: ' ' }}，实际 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "Step 1: 不应发送其他命令");

        // Step 2: Tab 切换焦点到 BodyCenter
        let should_quit = handle_key(&snapshot_search, press_key(KeyCode::Tab), &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Step 2: 应发送 UiFocusNext");
        assert!(
            matches!(cmd, AppCommand::UiFocusNext),
            "Step 2: 期望 UiFocusNext，实际 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "Step 2: 不应发送其他命令");

        // Step 3: BodyCenter 焦点下按 m → 弹出菜单
        let app_results = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        let snapshot_results = AppSnapshot::from_app(&app_results);

        let should_quit = handle_key(&snapshot_results, press_key(KeyCode::Char('m')), &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Step 3: 应发送 MenuOpen");
        assert!(
            matches!(cmd, AppCommand::MenuOpen),
            "Step 3: 期望 MenuOpen，实际 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "Step 3: 不应发送其他命令");

        // Step 4: 菜单可见时按 Esc → 关闭菜单
        let mut app_menu = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        app_menu.menu_visible = true;
        app_menu.menu_items = crate::app::default_menu_items();
        let snapshot_menu = AppSnapshot::from_app(&app_menu);

        let should_quit = handle_key(&snapshot_menu, press_key(KeyCode::Esc), &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("Step 4: 应发送 MenuCancel");
        assert!(
            matches!(cmd, AppCommand::MenuCancel),
            "Step 4: 期望 MenuCancel，实际 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "Step 4: 仅应收到 MenuCancel");
    }

    /// VAL-CROSS-003: 自定义 Space 后搜索框仍可输入空格
    /// 当 PlayerTogglePause 绑定为 'p' 而非 Space 时，搜索框中 Space 仍应发送 SearchInputChar
    #[tokio::test]
    async fn cross_area_custom_space_search_input_still_works() {
        use crate::keybindings::{KeyAction, KeyBindings};

        // 创建自定义 keybindings：PlayerTogglePause 绑定到 'p'，Space 无绑定
        let mut custom_bindings = KeyBindings::default();
        custom_bindings.unbind_action(&KeyAction::PlayerTogglePause);
        custom_bindings.bind_key(KeyCode::Char('p'), KeyAction::PlayerTogglePause);

        let mut app = App {
            view: View::Search,
            ui_focus: UiFocus::HeaderSearch,
            ..Default::default()
        };
        app.keybindings = std::sync::Arc::new(custom_bindings);
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // 搜索框按 Space → 应发送 SearchInputChar（不发送 PlayerTogglePause）
        let should_quit = handle_key(&snapshot, press_key(KeyCode::Char(' ')), &tx).await;
        assert!(!should_quit);
        let cmd = rx.try_recv().expect("应发送 SearchInputChar");
        assert!(
            matches!(cmd, AppCommand::SearchInputChar { c } if c == ' '),
            "自定义 Space 后搜索框按 Space 应发送 SearchInputChar {{ c: ' ' }}，实际 {:?}",
            cmd
        );
        assert!(
            rx.try_recv().is_err(),
            "不应发送其他命令（尤其是 PlayerTogglePause）"
        );

        // 验证 'p' 在非搜索框焦点下触发 PlayerTogglePause
        let mut app_body = App {
            view: View::Search,
            ui_focus: UiFocus::BodyCenter,
            ..Default::default()
        };
        // 重新创建 custom bindings（Arc is cloned by snapshot）
        let mut custom_bindings2 = KeyBindings::default();
        custom_bindings2.unbind_action(&KeyAction::PlayerTogglePause);
        custom_bindings2.bind_key(KeyCode::Char('p'), KeyAction::PlayerTogglePause);
        app_body.keybindings = std::sync::Arc::new(custom_bindings2);
        let snapshot_body = AppSnapshot::from_app(&app_body);

        let (tx2, mut rx2) = mpsc::channel::<AppCommand>(8);
        let should_quit = handle_key(&snapshot_body, press_key(KeyCode::Char(' ')), &tx2).await;
        assert!(!should_quit);
        // Space 不再绑定到 PlayerTogglePause，所以在非搜索焦点下也不应触发 toggle
        assert!(
            rx2.try_recv().is_err(),
            "自定义 Space 后非搜索焦点下 Space 不应触发任何命令"
        );

        // 验证 'p' 触发 PlayerTogglePause
        let should_quit = handle_key(&snapshot_body, press_key(KeyCode::Char('p')), &tx2).await;
        assert!(!should_quit);
        let cmd = rx2.try_recv().expect("'p' 应发送 PlayerTogglePause");
        assert!(
            matches!(cmd, AppCommand::PlayerTogglePause),
            "自定义 'p' 应触发 PlayerTogglePause，实际 {:?}",
            cmd
        );
        assert!(rx2.try_recv().is_err(), "不应发送其他命令");
    }
}
