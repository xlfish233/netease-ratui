use crate::app::{App, PlaylistMode, View};
use crate::messages::app::AppCommand;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

pub(super) async fn handle_key(app: &App, key: KeyEvent, tx: &mpsc::Sender<AppCommand>) -> bool {
    // Some terminals/platforms may report both press and release events; we only act on press/repeat.
    if matches!(key.kind, KeyEventKind::Release) {
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
            code: KeyCode::Tab, ..
        } => {
            let _ = tx.send(AppCommand::TabNext).await;
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

    match app.view {
        View::Login => {
            if app.login_cookie_input_visible {
                // Cookie 输入模式
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
                // 二维码登录模式
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
        View::Playlists => match key.code {
            KeyCode::Char('b') => {
                let _ = tx.send(AppCommand::Back).await;
            }
            KeyCode::Enter => {
                let _ = tx.send(AppCommand::PlaylistsOpenSelected).await;
            }
            KeyCode::Char('p') => {
                let _ = tx.send(AppCommand::PlaylistTracksPlaySelected).await;
            }
            KeyCode::Up => match app.playlist_mode {
                PlaylistMode::List => {
                    let _ = tx.send(AppCommand::PlaylistsMoveUp).await;
                }
                PlaylistMode::Tracks => {
                    let _ = tx.send(AppCommand::PlaylistTracksMoveUp).await;
                }
            },
            KeyCode::Down => match app.playlist_mode {
                PlaylistMode::List => {
                    let _ = tx.send(AppCommand::PlaylistsMoveDown).await;
                }
                PlaylistMode::Tracks => {
                    let _ = tx.send(AppCommand::PlaylistTracksMoveDown).await;
                }
            },
            _ => {}
        },
        View::Search => match key.code {
            KeyCode::Char('p') => {
                let _ = tx.send(AppCommand::SearchPlaySelected).await;
            }
            KeyCode::Enter => {
                let _ = tx.send(AppCommand::SearchSubmit).await;
            }
            KeyCode::Backspace => {
                let _ = tx.send(AppCommand::SearchInputBackspace).await;
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    let _ = tx.send(AppCommand::SearchInputChar { c }).await;
                }
            }
            KeyCode::Up => {
                let _ = tx.send(AppCommand::SearchMoveUp).await;
            }
            KeyCode::Down => {
                let _ = tx.send(AppCommand::SearchMoveDown).await;
            }
            _ => {}
        },
        View::Lyrics => match key.code {
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
        },
        View::Settings => match key.code {
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
        },
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tab_release_is_ignored() {
        let app = App::default();
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
    async fn tab_press_sends_tabnext_once() {
        let app = App::default();
        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let key = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let should_quit = handle_key(&app, key, &tx).await;
        assert!(!should_quit);
        assert!(matches!(rx.try_recv(), Ok(AppCommand::TabNext)));
        assert!(rx.try_recv().is_err());
    }
}
