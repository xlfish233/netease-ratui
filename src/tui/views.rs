use crate::app::{App, View, tab_configs, tab_index_for_view};
use crate::tui::login_view::draw_login;
use crate::tui::lyrics_view::draw_lyrics;
use crate::tui::playlists_view::draw_playlists;
use crate::tui::search_view::draw_search;
use crate::tui::settings_view::draw_settings;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Tabs},
    Frame,
};

pub(super) fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();

    let configs = tab_configs(app.logged_in);
    let titles: Vec<Line> = configs.iter().map(|c| Line::from(c.title.as_ref())).collect();
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
