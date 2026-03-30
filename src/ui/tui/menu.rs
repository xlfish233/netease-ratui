use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use crate::app::AppSnapshot;

/// Draw the action menu overlay centered on the canvas area.
pub(super) fn draw_menu_overlay(f: &mut Frame, area: Rect, app: &AppSnapshot) {
    let width = area.width.saturating_sub(4).min(40);
    let item_count = app.menu_items.len() as u16;
    let height = item_count
        .saturating_add(2)
        .min(area.height.saturating_sub(4)); // 2 for borders
    let popup = centered_rect(area, width, height);

    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .menu_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app.menu_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if i == app.menu_selected { " > " } else { "   " };
            ListItem::new(Line::from(Span::styled(format!("{prefix}{item}"), style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("操作菜单")
            .style(Style::default().fg(Color::Cyan)),
    );

    let mut state = ListState::default();
    state.select(Some(app.menu_selected));

    f.render_stateful_widget(list, popup, &mut state);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, default_menu_items};

    /// VAL-MENU-002: 操作菜单显示占位选项
    #[test]
    fn default_menu_items_has_at_least_4() {
        let items = default_menu_items();
        assert!(
            items.len() >= 4,
            "菜单应有至少 4 个选项，实际有 {} 个",
            items.len()
        );
        // Check that expected placeholder items exist
        assert!(items.iter().any(|i| i.contains("收藏")));
        assert!(items.iter().any(|i| i.contains("下载")));
    }

    /// Test that menu rendering doesn't panic with default state
    #[test]
    fn menu_render_does_not_panic() {
        let app = App::default();
        let snapshot = AppSnapshot::from_app(&app);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_menu_overlay(f, f.area(), &snapshot);
            })
            .unwrap();
    }
}
