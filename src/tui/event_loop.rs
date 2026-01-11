use crate::app::{App, View};
use crate::messages::app::{AppCommand, AppEvent};
use crate::tui::guard::TuiGuard;
use crate::tui::keyboard::handle_key;
use crate::tui::mouse::handle_mouse;
use crate::tui::views::draw_ui;
use crossterm::event::{self, Event};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub(super) async fn run_tui_internal(
    mut app: App,
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
                AppEvent::State(s) => app = *s,
                AppEvent::Toast(s) => match app.view {
                    View::Login => app.login_status = s,
                    View::Playlists => app.playlists_status = s,
                    View::Search => app.search_status = s,
                    View::Lyrics => app.lyrics_status = s,
                    View::Settings => app.settings_status = s,
                },
                AppEvent::Error(e) => match app.view {
                    View::Login => app.login_status = format!("错误: {e}"),
                    View::Playlists => app.playlists_status = format!("错误: {e}"),
                    View::Search => app.search_status = format!("错误: {e}"),
                    View::Lyrics => app.lyrics_status = format!("错误: {e}"),
                    View::Settings => app.settings_status = format!("错误: {e}"),
                },
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
