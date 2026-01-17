use super::styles::focus_style;
use crate::app::{PlaylistMode, PlaylistsSnapshot};
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub(super) fn draw_playlist_list(
    f: &mut Frame,
    area: Rect,
    state: &PlaylistsSnapshot,
    active: bool,
) {
    let border = focus_style(active);
    let items: Vec<ListItem> = state
        .playlists
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mark = if p.special_type == 5 || p.name.contains("我喜欢") {
                " ♥"
            } else {
                ""
            };
            ListItem::new(Line::from(format!(
                "{}. {}({}首){}",
                i + 1,
                p.name,
                p.track_count,
                mark
            )))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("歌单[2]")
                .border_style(border),
        )
        .highlight_style(Style::default().fg(Color::Yellow));

    let mut st = ratatui::widgets::ListState::default();
    if !state.playlists.is_empty() {
        st.select(Some(
            state
                .playlists_selected
                .min(state.playlists.len().saturating_sub(1)),
        ));
    }
    f.render_stateful_widget(list, area, &mut st);
}

pub(super) fn draw_playlists(f: &mut Frame, area: Rect, state: &PlaylistsSnapshot, active: bool) {
    let border = focus_style(active);
    if matches!(state.playlist_mode, PlaylistMode::Tracks) {
        let items: Vec<ListItem> = state
            .playlist_tracks
            .iter()
            .enumerate()
            .map(|(i, s)| ListItem::new(Line::from(format!("{}. {}-{}", i + 1, s.name, s.artists))))
            .collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("歌曲[3](↑↓选择 p 播放 b 返回)")
                    .border_style(border),
            )
            .highlight_style(Style::default().fg(Color::Yellow));

        let mut st = ratatui::widgets::ListState::default();
        if !state.playlist_tracks.is_empty() {
            st.select(Some(
                state
                    .playlist_tracks_selected
                    .min(state.playlist_tracks.len().saturating_sub(1)),
            ));
        }
        f.render_stateful_widget(list, area, &mut st);
    } else {
        let selected = state.playlists.get(state.playlists_selected);
        let hint = if let Some(p) = selected {
            format!("选中:{}({}首)\n回车打开歌单", p.name, p.track_count)
        } else {
            "暂无歌单，等待登录后加载".to_owned()
        };
        let text = Text::from(vec![
            Line::from(state.playlists_status.as_str()),
            Line::from(""),
            Line::from(hint),
        ]);
        let panel = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("歌单详情[3]")
                    .border_style(border),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(panel, area);
    }
}
