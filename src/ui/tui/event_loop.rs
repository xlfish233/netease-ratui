use super::guard::TuiGuard;
use super::keyboard::handle_key;
use super::mouse::handle_mouse;
use super::views::draw_ui;
use crate::app::{AppSnapshot, AppViewSnapshot, View};
use crate::messages::app::{AppCommand, AppEvent};
use crossterm::event::{self, Event};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub(super) async fn run_tui_internal(
    mut app: AppSnapshot,
    tx: mpsc::Sender<AppCommand>,
    mut rx: mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    let _guard = TuiGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let _ = tx.send(AppCommand::Bootstrap).await;

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        while let Ok(evt) = rx.try_recv() {
            match evt {
                AppEvent::State(s) => app = s,
                AppEvent::Toast(s) => apply_status_message(&mut app, s),
                AppEvent::Error(e) => apply_status_message(&mut app, format!("错误: {e}")),
            }
        }

        terminal.draw(|f| draw_ui(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key(&app, key, &tx).await {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse(&app, mouse, &tx).await;
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn apply_status_message(app: &mut AppSnapshot, message: String) {
    match (&app.view, &mut app.view_state) {
        (View::Login, AppViewSnapshot::Login(state)) => state.login_status = message,
        (View::Playlists, AppViewSnapshot::Playlists(state)) => state.playlists_status = message,
        (View::Search, AppViewSnapshot::Search(state)) => state.search_status = message,
        (View::Lyrics, AppViewSnapshot::Lyrics(state)) => state.lyrics_status = message,
        (View::Settings, AppViewSnapshot::Settings(state)) => state.settings_status = message,
        _ => {}
    }
}
