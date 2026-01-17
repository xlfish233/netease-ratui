use super::styles::focus_style;
use super::widgets::list_state;
use crate::app::SearchSnapshot;
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

pub(super) fn draw_search(f: &mut Frame, area: Rect, state: &SearchSnapshot, active: bool) {
    let border = focus_style(active);
    let items = state
        .search_results
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let line = format!("{}. {}-{}({})", s.id, s.name, s.artists, i + 1);
            ListItem::new(Line::from(line))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("结果[3](↑↓选择)")
                .border_style(border),
        )
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_stateful_widget(list, area, &mut list_state(state.search_selected));
}
