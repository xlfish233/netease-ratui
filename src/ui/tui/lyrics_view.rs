use super::styles::focus_style;
use super::utils::{apply_lyrics_offset, current_lyric_index, playback_time_ms};
use super::widgets::list_state;
use crate::app::{LyricsSnapshot, PlayerSnapshot};
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub(super) fn draw_lyrics(
    f: &mut Frame,
    area: Rect,
    state: &LyricsSnapshot,
    player: &PlayerSnapshot,
    active: bool,
) {
    let border = focus_style(active);
    if state.lyrics.is_empty() {
        let block = Paragraph::new(state.lyrics_status.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("歌词")
                    .border_style(border),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(block, area);
        return;
    }

    let (elapsed_ms, _) = playback_time_ms(player);
    let selected = if state.lyrics_follow {
        current_lyric_index(
            &state.lyrics,
            apply_lyrics_offset(elapsed_ms, state.lyrics_offset_ms),
        )
        .unwrap_or(0)
    } else {
        state
            .lyrics_selected
            .min(state.lyrics.len().saturating_sub(1))
    };

    let items = state
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

    // Keep about 5 lines of context around the highlighted lyric line.
    let scroll_padding = 5.min(area.height.saturating_sub(2) as usize / 2);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("歌词（自动滚动）")
                .border_style(border),
        )
        .scroll_padding(scroll_padding)
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_stateful_widget(list, area, &mut list_state(selected));
}
