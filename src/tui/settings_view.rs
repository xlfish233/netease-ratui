use crate::app::App;
use crate::tui::player_status::draw_player_status;
use crate::tui::utils::{br_label, fmt_offset, play_mode_label};
use crate::tui::widgets::list_state;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

const PLAYER_PANEL_HEIGHT: u16 = 12;

pub(super) fn draw_settings(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(PLAYER_PANEL_HEIGHT)])
        .split(area);

    let items = vec![
        ListItem::new(Line::from(format!("音质: {}", br_label(app.play_br)))),
        ListItem::new(Line::from(format!("音量: {:.0}%", app.volume * 100.0))),
        ListItem::new(Line::from(format!(
            "播放模式: {}",
            play_mode_label(app.play_mode)
        ))),
        ListItem::new(Line::from(format!(
            "歌词 offset: {}",
            fmt_offset(app.lyrics_offset_ms)
        ))),
        ListItem::new(Line::from("清除音频缓存".to_owned())),
        ListItem::new(Line::from(if app.logged_in {
            "退出登录".to_owned()
        } else {
            "退出登录（未登录）".to_owned()
        })),
    ];

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("设置（↑↓选择，←→调整，Enter 操作）"),
        )
        .highlight_style(Style::default().fg(Color::Yellow));

    f.render_stateful_widget(list, chunks[0], &mut list_state(app.settings_selected));

    draw_player_status(
        f,
        chunks[1],
        app,
        "状态",
        "设置",
        app.settings_status.as_str(),
    );
}
