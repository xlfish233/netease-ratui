use crate::app::{App, tab_configs};
use crate::messages::app::AppCommand;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::mpsc;
use unicode_width::UnicodeWidthStr;

pub(super) async fn handle_mouse(
    app: &App,
    mouse: MouseEvent,
    tx: &mpsc::Sender<AppCommand>,
) {
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        && mouse.row < 3  // 页签区域高度为 3 行
        && let Some(tab_index) = calculate_tab_index(app, mouse.column)
    {
        let _ = tx.send(AppCommand::TabTo { index: tab_index }).await;
    }
}

pub(super) fn calculate_tab_index(app: &App, column: u16) -> Option<usize> {
    let configs = tab_configs(app.logged_in);

    const DIVIDER_WIDTH: u16 = 1;
    const PADDING_LEFT_WIDTH: u16 = 1;
    const PADDING_RIGHT_WIDTH: u16 = 1;

    let mut x = 1u16;

    for (i, cfg) in configs.iter().enumerate() {
        let title_width = cfg.title.width() as u16;
        let divider_width = if i < configs.len() - 1 {
            DIVIDER_WIDTH
        } else {
            0
        };

        let tab_start = x;
        let tab_end = x
            .saturating_add(PADDING_LEFT_WIDTH)
            .saturating_add(title_width)
            .saturating_add(PADDING_RIGHT_WIDTH);

        if column >= tab_start && column < tab_end {
            return Some(i);
        }

        x = tab_end.saturating_add(divider_width);
    }

    None
}
