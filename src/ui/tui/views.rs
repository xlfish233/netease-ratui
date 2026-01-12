use super::login_view::draw_login;
use super::lyrics_view::draw_lyrics;
use super::playlists_view::draw_playlists;
use super::search_view::draw_search;
use super::settings_view::draw_settings;
use crate::app::{AppSnapshot, AppViewSnapshot, View, tab_configs, tab_index_for_view};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Tabs},
};

pub(super) fn draw_ui(f: &mut Frame, app: &AppSnapshot) {
    let size = f.area();

    let configs = tab_configs(app.logged_in);
    let titles: Vec<Line> = configs.iter().map(|c| Line::from(c.title)).collect();
    let selected = tab_index_for_view(app.view, app.logged_in).unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .divider("|")
        .padding(" ", " ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("netease-ratui"),
        )
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Yellow));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(size);
    f.render_widget(tabs, chunks[0]);

    match (&app.view, &app.view_state) {
        (View::Login, AppViewSnapshot::Login(state)) => {
            draw_login(f, chunks[1], state, app.logged_in);
        }
        (View::Playlists, AppViewSnapshot::Playlists(state)) => {
            draw_playlists(f, chunks[1], state, &app.player);
        }
        (View::Search, AppViewSnapshot::Search(state)) => {
            draw_search(f, chunks[1], state, &app.player);
        }
        (View::Lyrics, AppViewSnapshot::Lyrics(state)) => {
            draw_lyrics(f, chunks[1], state, &app.player);
        }
        (View::Settings, AppViewSnapshot::Settings(state)) => {
            draw_settings(f, chunks[1], state, &app.player, app.logged_in);
        }
        _ => {}
    }
}
