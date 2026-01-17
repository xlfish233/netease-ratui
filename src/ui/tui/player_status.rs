use super::utils::{br_label, fmt_mmss, playback_time_ms};
use super::widgets::progress_bar_text;
use crate::app::{PlayMode, PlayerSnapshot};
use ratatui::{
    Frame,
    prelude::Rect,
    text::{Line, Text},
    widgets::Paragraph,
};

pub(super) fn draw_footer(f: &mut Frame, area: Rect, player: &PlayerSnapshot, view_status: &str) {
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

    let lines = vec![
        Line::from(format!("提示:{view_status}|Now:{now}")),
        Line::from(format!(
            "时间: {time_text} | 模式: {mode_text} | 音量: {:.0}% | 音质: {} | {progress}",
            (player.volume.clamp(0.0, 2.0) * 100.0),
            br_label(player.play_br),
        )),
        Line::from(
            "1-4 切换页 | Tab 焦点 | q 退出 | ? 帮助 | 空格 播放/暂停 | [/] 上一首/下一首 | Ctrl+Left/Right Seek | Alt+Up/Down 音量 | M 模式",
        ),
    ];

    let footer = Paragraph::new(Text::from(lines));
    f.render_widget(footer, area);
}
