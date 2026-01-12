use super::player_status::draw_player_status;
use super::utils::{apply_lyrics_offset, current_lyric_index, fmt_offset, playback_time_ms};
use super::widgets::list_state;
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

const PLAYER_PANEL_HEIGHT: u16 = 12;

pub(super) fn draw_lyrics(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(PLAYER_PANEL_HEIGHT)])
        .split(area);

    let offset_text = fmt_offset(app.lyrics_offset_ms);
    let mode_text = if app.lyrics_follow {
        "跟随"
    } else {
        "锁定"
    };
    let status_text = format!(
        "{} | {} | offset={}",
        app.lyrics_status, mode_text, offset_text
    );

    if app.lyrics.is_empty() {
        let block = Paragraph::new(app.lyrics_status.as_str())
            .block(Block::default().borders(Borders::ALL).title("歌词"))
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
    } else {
        let (elapsed_ms, _) = playback_time_ms(app);
        let selected = if app.lyrics_follow {
            current_lyric_index(
                &app.lyrics,
                apply_lyrics_offset(elapsed_ms, app.lyrics_offset_ms),
            )
            .unwrap_or(0)
        } else {
            app.lyrics_selected.min(app.lyrics.len().saturating_sub(1))
        };

        let items = app
            .lyrics
            .iter()
            .map(|l| {
                let mut lines = vec![Line::from(l.text.as_str())];
                if let Some(t) = l.translation.as_deref()
                    && !t.trim().is_empty()
                {
                    lines.push(Line::from(format!("  {t}")));
                }
                ListItem::new(Text::from(lines).centered())
            })
            .collect::<Vec<_>>();

        // 计算合适的 scroll_padding：让高亮行前后各显示约 5 行
        let scroll_padding = 5.min(chunks[0].height.saturating_sub(2) as usize / 2);

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("歌词（自动滚动）"),
            )
            .scroll_padding(scroll_padding)
            .highlight_style(Style::default().fg(Color::Yellow));
        f.render_stateful_widget(list, chunks[0], &mut list_state(selected));
    }

    draw_player_status(f, chunks[1], app, "状态", "歌词", status_text.as_str());
}
