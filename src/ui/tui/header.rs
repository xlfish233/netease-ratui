use super::layout::HeaderLayout;
use crate::app::{AppSnapshot, UiFocus, tab_configs, tab_index_for_view};
use ratatui::{
    Frame,
    style::{Color, Style},
    text::Line,
    widgets::{Paragraph, Tabs},
};

pub(super) fn draw_header(f: &mut Frame, layout: &HeaderLayout, app: &AppSnapshot) {
    let configs = tab_configs(app.logged_in);
    let titles: Vec<Line> = configs
        .iter()
        .enumerate()
        .map(|(i, c)| Line::from(format!("{}[F{}]", c.title, i + 1)))
        .collect();
    let selected = tab_index_for_view(app.view, app.logged_in).unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .divider("|")
        .padding(" ", " ")
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_widget(tabs, layout.tabs);

    let search_hint = if app.search_input.is_empty() {
        "Search[1]: (type and Enter)".to_owned()
    } else {
        format!("Search[1]: {}", app.search_input)
    };
    let search_style = if matches!(app.ui_focus, UiFocus::HeaderSearch) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    f.render_widget(
        Paragraph::new(search_hint).style(search_style),
        layout.search,
    );

    let focus_label = match app.ui_focus {
        UiFocus::HeaderSearch => "Search",
        UiFocus::BodyLeft => "Left",
        UiFocus::BodyCenter => "Center",
        UiFocus::BodyRight => "Right",
    };
    let status = format!(
        "View:{}|Focus:{}|Login:{}|Help:{}",
        configs.get(selected).map(|c| c.title).unwrap_or(""),
        focus_label,
        if app.logged_in { "Yes" } else { "No" },
        if app.help_visible { "On" } else { "Off" }
    );
    f.render_widget(Paragraph::new(status), layout.status);
}
