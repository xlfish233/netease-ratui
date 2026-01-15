use ratatui::{
    Frame,
    prelude::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub(super) fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let width = area.width.saturating_sub(4).min(90);
    let height = area.height.saturating_sub(4).min(18);
    let popup = centered_rect(area, width, height);

    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from("Help"),
        Line::from(""),
        Line::from("1-4: Switch view"),
        Line::from("Tab / Shift+Tab: Focus cycle"),
        Line::from("Enter: Confirm / Open"),
        Line::from("Space: Play / Pause"),
        Line::from("[ / ]: Prev / Next"),
        Line::from("Ctrl+←/→: Seek"),
        Line::from("Alt+↑/↓: Volume"),
        Line::from("M: Play mode"),
        Line::from("? / Esc: Close help"),
    ];
    let help = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title("帮助"))
        .wrap(Wrap { trim: false });
    f.render_widget(help, popup);
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
