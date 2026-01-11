use crate::app::App;
use crate::tui::player_status::draw_player_status;
use crate::tui::widgets::list_state;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

const PLAYER_PANEL_HEIGHT: u16 = 12;

pub(super) fn draw_search(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(PLAYER_PANEL_HEIGHT),
        ])
        .split(area);

    let input = Paragraph::new(app.search_input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("关键词(回车搜索)"),
    );
    f.render_widget(input, chunks[0]);

    let items = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let line = format!("{}  {} - {}  ({})", s.id, s.name, s.artists, i + 1);
            ListItem::new(Line::from(line))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("结果(↑↓选择)"))
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_stateful_widget(list, chunks[1], &mut list_state(app.search_selected));

    draw_player_status(
        f,
        chunks[2],
        app,
        "状态",
        "搜索",
        app.search_status.as_str(),
    );
}
