use super::login_view::draw_login;
use super::lyrics_view::draw_lyrics;
use super::playlists_view::draw_playlists;
use super::search_view::draw_search;
use super::settings_view::draw_settings;
use crate::app::{App, View, tab_configs, tab_index_for_view};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Tabs},
};

pub(super) fn draw_ui(f: &mut Frame, app: &App) {
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

    match app.view {
        View::Login => draw_login(f, chunks[1], app),
        View::Playlists => draw_playlists(f, chunks[1], app),
        View::Search => draw_search(f, chunks[1], app),
        View::Lyrics => draw_lyrics(f, chunks[1], app),
        View::Settings => draw_settings(f, chunks[1], app),
    }
}
