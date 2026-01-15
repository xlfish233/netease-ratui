use ratatui::style::{Color, Style};

pub(super) fn focus_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    }
}
