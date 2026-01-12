use super::player_status::draw_player_status;
use super::utils::{br_label, fmt_offset, play_mode_label};
use super::widgets::list_state;
use crate::app::{PlayerSnapshot, SettingsSnapshot};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

const PLAYER_PANEL_HEIGHT: u16 = 12;

pub(super) fn draw_settings(
    f: &mut Frame,
    area: Rect,
    state: &SettingsSnapshot,
    player: &PlayerSnapshot,
    logged_in: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(PLAYER_PANEL_HEIGHT)])
        .split(area);

    let items = vec![
        ListItem::new(Line::from(format!("音质: {}", br_label(player.play_br)))),
        ListItem::new(Line::from(format!("音量: {:.0}%", player.volume * 100.0))),
        ListItem::new(Line::from(format!(
            "播放模式: {}",
            play_mode_label(player.play_mode)
        ))),
        ListItem::new(Line::from(format!(
            "歌词 offset: {}",
            fmt_offset(state.lyrics_offset_ms)
        ))),
        ListItem::new(Line::from("清除音频缓存".to_owned())),
        ListItem::new(Line::from(if logged_in {
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

    f.render_stateful_widget(list, chunks[0], &mut list_state(state.settings_selected));

    draw_player_status(
        f,
        chunks[1],
        player,
        "状态",
        "设置",
        state.settings_status.as_str(),
    );
}
