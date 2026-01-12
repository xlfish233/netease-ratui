use super::utils::{br_label, fmt_mmss, playback_time_ms};
use super::widgets::progress_bar_text;
use crate::app::{PlayMode, PlayerSnapshot};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(super) fn draw_player_status(
    f: &mut Frame,
    area: Rect,
    player: &PlayerSnapshot,
    title: &str,
    context_label: &str,
    context_value: &str,
) {
    let status_height = if area.height > 3 {
        6.min(area.height.saturating_sub(3))
    } else {
        area.height
    };
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(status_height), Constraint::Min(0)])
        .split(area);

    let now = player.now_playing.as_deref().unwrap_or("-");
    let (elapsed_ms, total_ms) = playback_time_ms(player);
    let progress = progress_bar_text(elapsed_ms, total_ms, 24);
    let time_text = format!(
        "{} / {}{}",
        fmt_mmss(elapsed_ms),
        total_ms.map(fmt_mmss).unwrap_or_else(|| "--:--".to_owned()),
        if player.paused { " (暂停)" } else { "" }
    );
    let mode_text = match player.play_mode {
        PlayMode::Sequential => "顺序",
        PlayMode::ListLoop => "列表循环",
        PlayMode::SingleLoop => "单曲循环",
        PlayMode::Shuffle => "随机",
    };

    let status_lines = vec![
        Line::from(format!("{context_label}: {context_value}")),
        Line::from(format!("播放: {} | Now: {}", player.play_status, now)),
        Line::from(format!(
            "时间: {} | 模式: {} | 音量: {:.0}% | 音质: {}",
            time_text,
            mode_text,
            (player.volume.clamp(0.0, 2.0) * 100.0),
            br_label(player.play_br),
        )),
        Line::from(progress),
    ];
    let status = Paragraph::new(Text::from(status_lines))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(status, status_chunks[0]);

    // 底栏帮助提示
    let help_lines = vec![
        Line::from("Tab 切换页 | q 退出"),
        Line::from("↑↓ 选择/滚动 | Enter 打开/确认"),
        Line::from("空格 暂停/继续 | Ctrl+S 停止 | [/] 上一首/下一首"),
        Line::from("Ctrl+←/→ Seek | Alt+↑/↓ 音量 | M 切换模式"),
        Line::from("歌词: o 跟随/锁定 | g 当前行"),
        Line::from("歌词 offset: Alt+←/→ ±200ms | Shift+Alt+←/→ ±50ms"),
        Line::from("设置: ↑↓选择 | ←→调整 | Enter 操作（含退出登录）"),
    ];
    let help = Paragraph::new(Text::from(help_lines))
        .block(Block::default().borders(Borders::ALL).title("帮助"))
        .wrap(Wrap { trim: false });
    f.render_widget(help, status_chunks[1]);
}
