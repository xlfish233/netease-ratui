use crate::app::{AppSnapshot, AppViewSnapshot, PlaylistMode, UiFocus, View};
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
            KeyCode::Char('?') | KeyCode::Esc => {
                let _ = tx.send(AppCommand::UiToggleHelp).await;
            }
            _ => {}
        }
        return false;
    }

    match key {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        } => {
            let _ = tx.send(AppCommand::Quit).await;
            return true;
        }
        KeyEvent {
            code: KeyCode::Char('?'),
            ..
        } => {
            let _ = tx.send(AppCommand::UiToggleHelp).await;
            return false;
        }
        KeyEvent {
            code: KeyCode::Tab,
            modifiers,
            ..
        } => {
            if modifiers.contains(KeyModifiers::CONTROL) {
                let _ = tx.send(AppCommand::TabNext).await;
            } else {
                let _ = tx.send(AppCommand::UiFocusNext).await;
            }
        }
        KeyEvent {
            code: KeyCode::BackTab,
            ..
        } => {
            let _ = tx.send(AppCommand::UiFocusPrev).await;
        }
        _ => {}
    }

    // Global player controls (avoid interfering with text input as much as possible)
    match (key.code, key.modifiers) {
        (KeyCode::Char(' '), _) => {
            let _ = tx.send(AppCommand::PlayerTogglePause).await;
            return false;
        }
        (KeyCode::Char('['), _) => {
            let _ = tx.send(AppCommand::PlayerPrev).await;
            return false;
        }
        (KeyCode::Char(']'), _) => {
            let _ = tx.send(AppCommand::PlayerNext).await;
            return false;
        }
        (KeyCode::Char('M'), _) => {
            let _ = tx.send(AppCommand::PlayerCycleMode).await;
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
                    _ => {}
                },
                _ => {}
            }
        }
        View::Search => {
            match (focus, key.code) {
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
                _ => {}
            }
        }
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
            if focus != UiFocus::BodyCenter {
                return false;
            }
            match key.code {
                KeyCode::Up => {
                    let _ = tx.send(AppCommand::SettingsMoveUp).await;
                }
                KeyCode::Down => {
                    let _ = tx.send(AppCommand::SettingsMoveDown).await;
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
                _ => {}
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

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
}
