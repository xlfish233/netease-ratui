use super::styles::focus_style;
use super::utils::{br_label, fmt_offset, play_mode_label};
use super::widgets::list_state;
use crate::app::{PlayerSnapshot, SettingsSnapshot};
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

pub(super) fn draw_settings(
    f: &mut Frame,
    area: Rect,
    state: &SettingsSnapshot,
    player: &PlayerSnapshot,
    logged_in: bool,
    active: bool,
) {
    let border = focus_style(active);
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
        ListItem::new(Line::from(format!(
            "淡入淡出: {}",
            if state.crossfade_ms == 0 {
                "关闭".to_owned()
            } else {
                format!("{}ms", state.crossfade_ms)
            }
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
                .title("设置（↑↓选择，←→调整，Enter 操作）")
                .border_style(border),
        )
        .highlight_style(Style::default().fg(Color::Yellow));

    f.render_stateful_widget(list, area, &mut list_state(state.settings_selected));
}
