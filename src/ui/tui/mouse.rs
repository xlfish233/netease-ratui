use super::utils::canvas_rect;
use crate::app::{AppSnapshot, AppViewSnapshot, PlaylistMode, View, tab_configs};
use crate::messages::app::AppCommand;
use crossterm::{
    event::{MouseButton, MouseEvent, MouseEventKind},
    terminal,
};
use ratatui::layout::Rect;
use tokio::sync::mpsc;
use unicode_width::UnicodeWidthStr;

/// Layout constants matching layout.rs
const HEADER_HEIGHT: u16 = 3;
const FOOTER_HEIGHT: u16 = 3;
const TOAST_HEIGHT: u16 = 3;
const LEFT_WIDTH: u16 = 24;
const CENTER_WIDTH: u16 = 58;

pub(super) async fn handle_mouse(
    app: &AppSnapshot,
    mouse: MouseEvent,
    tx: &mpsc::Sender<AppCommand>,
) {
    if app.help_visible {
        return;
    }
    let Ok((cols, rows)) = terminal::size() else {
        return;
    };
    let Some(canvas) = canvas_rect(Rect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    }) else {
        return;
    };

    handle_mouse_with_canvas(app, mouse, &canvas, tx).await;
}

/// Core mouse handling logic, separated for testability.
/// Takes a pre-computed canvas rect so tests don't need terminal::size().
async fn handle_mouse_with_canvas(
    app: &AppSnapshot,
    mouse: MouseEvent,
    canvas: &Rect,
    tx: &mpsc::Sender<AppCommand>,
) {
    if mouse.column < canvas.x
        || mouse.row < canvas.y
        || mouse.column >= canvas.x + canvas.width
        || mouse.row >= canvas.y + canvas.height
    {
        return;
    }

    let column = mouse.column - canvas.x;
    let row = mouse.row - canvas.y;

    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return;
    }

    // Tabs row (row 0 within header)
    if row == 0 {
        if let Some(tab_index) = calculate_tab_index(app, column) {
            let _ = tx.send(AppCommand::TabTo { index: tab_index }).await;
        }
        return;
    }

    // Body area: rows after header, before footer+toast
    let body_start = HEADER_HEIGHT;
    let body_end = canvas.height.saturating_sub(FOOTER_HEIGHT + TOAST_HEIGHT);
    if row < body_start || row >= body_end {
        return;
    }

    // Determine which body panel was clicked
    if column < LEFT_WIDTH {
        // Left panel click
        handle_left_panel_click(app, row - body_start, tx).await;
    } else if column < LEFT_WIDTH + CENTER_WIDTH {
        // Center panel click
        handle_center_panel_click(app, row - body_start, tx).await;
    }
    // Right panel: no list click action
}

/// Calculate list item index from a mouse row within a panel.
/// Accounts for the border (top border = 1 row).
/// Returns None if the row is outside the list content area.
fn row_to_item_index(row_in_panel: u16, item_count: usize) -> Option<usize> {
    // Border offset: top border = 1 row
    let content_row = row_in_panel as usize;
    if content_row == 0 {
        return None; // Top border
    }
    let index = content_row - 1; // Subtract top border
    if index < item_count {
        Some(index)
    } else {
        None
    }
}

async fn handle_left_panel_click(app: &AppSnapshot, row: u16, tx: &mpsc::Sender<AppCommand>) {
    if app.view == View::Playlists
        && let AppViewSnapshot::Playlists(state) = &app.view_state
    {
        let count = state.playlists.len();
        if let Some(index) = row_to_item_index(row, count) {
            let _ = tx.send(AppCommand::PlaylistsMoveTo { index }).await;
        }
    }
}

async fn handle_center_panel_click(app: &AppSnapshot, row: u16, tx: &mpsc::Sender<AppCommand>) {
    match (&app.view, &app.view_state) {
        (View::Playlists, AppViewSnapshot::Playlists(state)) => {
            match state.playlist_mode {
                PlaylistMode::Tracks => {
                    let count = state.playlist_tracks.len();
                    if let Some(index) = row_to_item_index(row, count) {
                        let _ = tx.send(AppCommand::PlaylistTracksMoveTo { index }).await;
                    }
                }
                PlaylistMode::List => {
                    // In List mode, center panel shows playlist detail (not a list)
                }
            }
        }
        (View::Search, AppViewSnapshot::Search(state)) => {
            let count = state.search_results.len();
            if let Some(index) = row_to_item_index(row, count) {
                let _ = tx.send(AppCommand::SearchMoveTo { index }).await;
            }
        }
        _ => {}
    }
}

pub(super) fn calculate_tab_index(app: &AppSnapshot, column: u16) -> Option<usize> {
    let configs = tab_configs(app.logged_in);

    const DIVIDER_WIDTH: u16 = 1;
    const PADDING_LEFT_WIDTH: u16 = 1;
    const PADDING_RIGHT_WIDTH: u16 = 1;

    let mut x = 0u16;

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

/// Calculate canvas-local coordinates for testing.
/// Given a canvas origin (canvas_x, canvas_y), returns a MouseEvent
/// positioned at the specified (local_col, local_row) within the canvas.
#[cfg(test)]
fn make_mouse_event(local_col: u16, local_row: u16, canvas_x: u16, canvas_y: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: canvas_x + local_col,
        row: canvas_y + local_row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Song};
    use crate::domain::model::Playlist;
    use crate::ui::tui::utils::{MIN_CANVAS_HEIGHT, MIN_CANVAS_WIDTH};

    /// Helper: create a test canvas at origin (0, 0) with minimum dimensions.
    fn test_canvas() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: MIN_CANVAS_WIDTH,
            height: MIN_CANVAS_HEIGHT,
        }
    }

    /// Helper: run handle_mouse with the test canvas
    async fn run_mouse(app: &AppSnapshot, mouse: MouseEvent, tx: &mpsc::Sender<AppCommand>) {
        let canvas = test_canvas();
        handle_mouse_with_canvas(app, mouse, &canvas, tx).await;
    }

    /// VAL-MOUSE-001: 左键单击歌单列表项选中
    #[tokio::test]
    async fn click_playlist_list_selects_item() {
        let mut app = App {
            view: View::Playlists,
            logged_in: true,
            ..Default::default()
        };
        app.playlists = vec![
            Playlist {
                id: 1,
                name: "我喜欢的音乐".to_owned(),
                track_count: 100,
                special_type: 5,
            },
            Playlist {
                id: 2,
                name: "歌单B".to_owned(),
                track_count: 50,
                special_type: 0,
            },
            Playlist {
                id: 3,
                name: "歌单C".to_owned(),
                track_count: 30,
                special_type: 0,
            },
        ];
        app.playlists_selected = 0;
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click on left panel (col 0-23), row in body
        // Body starts at row 3 (HEADER_HEIGHT)
        // Within panel, row 0 = top border, row 1 = item 0, row 2 = item 1, row 3 = item 2
        let mouse = make_mouse_event(5, HEADER_HEIGHT + 3, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        let cmd = rx.try_recv().expect("应发送 PlaylistsMoveTo 命令");
        assert!(
            matches!(cmd, AppCommand::PlaylistsMoveTo { index } if index == 2),
            "期望 PlaylistsMoveTo {{ index: 2 }}，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-MOUSE-002: 左键单击歌曲列表项选中
    #[tokio::test]
    async fn click_playlist_tracks_selects_item() {
        let mut app = App {
            view: View::Playlists,
            logged_in: true,
            ui_focus: crate::app::UiFocus::BodyCenter,
            ..Default::default()
        };
        app.playlist_mode = PlaylistMode::Tracks;
        app.playlist_tracks = vec![
            Song {
                id: 1,
                name: "Song A".to_owned(),
                artists: "Artist A".to_owned(),
            },
            Song {
                id: 2,
                name: "Song B".to_owned(),
                artists: "Artist B".to_owned(),
            },
            Song {
                id: 3,
                name: "Song C".to_owned(),
                artists: "Artist C".to_owned(),
            },
            Song {
                id: 4,
                name: "Song D".to_owned(),
                artists: "Artist D".to_owned(),
            },
        ];
        app.playlist_tracks_selected = 0;
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click on center panel (col 24+), row within body
        // Body starts at row 3, center panel starts at col 24
        // Within panel, row 0 = top border, row 1 = item 0, row 2 = item 1, ...
        // Click on item 3 (Song D): row = HEADER_HEIGHT + 4 = 7
        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT + 4, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        let cmd = rx.try_recv().expect("应发送 PlaylistTracksMoveTo 命令");
        assert!(
            matches!(cmd, AppCommand::PlaylistTracksMoveTo { index } if index == 3),
            "期望 PlaylistTracksMoveTo {{ index: 3 }}，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// VAL-MOUSE-003: 左键单击搜索结果选中
    #[tokio::test]
    async fn click_search_result_selects_item() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![
            Song {
                id: 1,
                name: "Result A".to_owned(),
                artists: "Artist A".to_owned(),
            },
            Song {
                id: 2,
                name: "Result B".to_owned(),
                artists: "Artist B".to_owned(),
            },
            Song {
                id: 3,
                name: "Result C".to_owned(),
                artists: "Artist C".to_owned(),
            },
        ];
        app.search_selected = 0;
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click on center panel, item index 1 (Result B)
        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT + 2, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        let cmd = rx.try_recv().expect("应发送 SearchMoveTo 命令");
        assert!(
            matches!(cmd, AppCommand::SearchMoveTo { index } if index == 1),
            "期望 SearchMoveTo {{ index: 1 }}，实际收到 {:?}",
            cmd
        );
        assert!(rx.try_recv().is_err(), "不应发送其他命令");
    }

    /// Click on top border row should not select any item
    #[tokio::test]
    async fn click_on_border_row_does_not_select() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![Song {
            id: 1,
            name: "Result A".to_owned(),
            artists: "Artist A".to_owned(),
        }];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click on the top border row (row 0 within panel = row HEADER_HEIGHT in canvas)
        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(rx.try_recv().is_err(), "点击边框行不应发送命令");
    }

    /// Click below the last item should not select
    #[tokio::test]
    async fn click_below_items_does_not_select() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![Song {
            id: 1,
            name: "Result A".to_owned(),
            artists: "Artist A".to_owned(),
        }];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click 5 rows below body_start (only 1 item, so row 1+5 = way past items)
        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT + 6, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(rx.try_recv().is_err(), "点击列表底部之外不应发送命令");
    }

    /// Click on right panel should not trigger any list selection
    #[tokio::test]
    async fn click_right_panel_does_not_select() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![Song {
            id: 1,
            name: "Result A".to_owned(),
            artists: "Artist A".to_owned(),
        }];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Click on right panel (col >= LEFT_WIDTH + CENTER_WIDTH)
        let mouse = make_mouse_event(LEFT_WIDTH + CENTER_WIDTH + 5, HEADER_HEIGHT + 1, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(rx.try_recv().is_err(), "右侧面板点击不应发送命令");
    }

    /// Click in footer area should not trigger anything
    #[tokio::test]
    async fn click_footer_does_not_trigger() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![Song {
            id: 1,
            name: "Result A".to_owned(),
            artists: "Artist A".to_owned(),
        }];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        // Footer starts at canvas.height - FOOTER_HEIGHT = 29 - 3 = 26
        let mouse = make_mouse_event(LEFT_WIDTH + 10, MIN_CANVAS_HEIGHT - FOOTER_HEIGHT, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(rx.try_recv().is_err(), "页脚区域点击不应发送命令");
    }

    /// Empty list should not trigger selection
    #[tokio::test]
    async fn click_empty_list_does_not_select() {
        let mut app = App {
            view: View::Search,
            logged_in: true,
            ..Default::default()
        };
        app.search_results = vec![];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT + 1, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(rx.try_recv().is_err(), "空列表点击不应发送命令");
    }

    /// Playlists List mode center panel click should not trigger (it shows detail, not a list)
    #[tokio::test]
    async fn click_playlists_list_mode_center_does_not_select() {
        let mut app = App {
            view: View::Playlists,
            logged_in: true,
            ..Default::default()
        };
        app.playlist_mode = PlaylistMode::List;
        app.playlists = vec![Playlist {
            id: 1,
            name: "test".to_owned(),
            track_count: 10,
            special_type: 0,
        }];
        let snapshot = AppSnapshot::from_app(&app);

        let (tx, mut rx) = mpsc::channel::<AppCommand>(8);

        let mouse = make_mouse_event(LEFT_WIDTH + 10, HEADER_HEIGHT + 1, 0, 0);
        run_mouse(&snapshot, mouse, &tx).await;

        assert!(
            rx.try_recv().is_err(),
            "歌单列表模式下中间面板点击不应发送命令"
        );
    }
}
