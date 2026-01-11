use crate::app::{App, PlaylistMode};
use crate::tui::player_status::draw_player_status;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

const PLAYER_PANEL_HEIGHT: u16 = 12;

pub(super) fn draw_playlists(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(PLAYER_PANEL_HEIGHT)])
        .split(area);

    let title = match app.playlist_mode {
        PlaylistMode::List => "歌单(↑↓选择 回车打开)",
        PlaylistMode::Tracks => "歌曲(↑↓选择 p 播放 b 返回)",
    };
    let items: Vec<ListItem> = match app.playlist_mode {
        PlaylistMode::List => app
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
                    "{}  {} ({}首){}",
                    i + 1,
                    p.name,
                    p.track_count,
                    mark
                )))
            })
            .collect(),
        PlaylistMode::Tracks => app
            .playlist_tracks
            .iter()
            .enumerate()
            .map(|(i, s)| {
                ListItem::new(Line::from(format!("{}  {} - {}", i + 1, s.name, s.artists)))
            })
            .collect(),
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().fg(Color::Yellow));

    let mut st = ratatui::widgets::ListState::default();
    let sel = match app.playlist_mode {
        PlaylistMode::List => app.playlists_selected,
        PlaylistMode::Tracks => app.playlist_tracks_selected,
    };
    st.select(Some(sel));
    f.render_stateful_widget(list, chunks[0], &mut st);

    draw_player_status(
        f,
        chunks[1],
        app,
        "状态",
        "歌单",
        app.playlists_status.as_str(),
    );
}
