use crate::app::{AppSnapshot, AppViewSnapshot, PlayMode, PlayerSnapshot, View};
use ratatui::layout::Rect;

pub(super) const MIN_CANVAS_WIDTH: u16 = 122;
pub(super) const MIN_CANVAS_HEIGHT: u16 = 29;

pub(super) fn is_unauth_login_page(app: &AppSnapshot) -> bool {
    !app.logged_in
        && matches!(app.view, View::Login)
        && matches!(app.view_state, AppViewSnapshot::Login(_))
}

pub(super) fn canvas_rect(area: Rect) -> Option<Rect> {
    if area.width < MIN_CANVAS_WIDTH || area.height < MIN_CANVAS_HEIGHT {
        return None;
    }

    let x = area.x + (area.width - MIN_CANVAS_WIDTH) / 2;
    let y = area.y + (area.height - MIN_CANVAS_HEIGHT) / 2;
    Some(Rect {
        x,
        y,
        width: MIN_CANVAS_WIDTH,
        height: MIN_CANVAS_HEIGHT,
    })
}

pub(super) fn playback_time_ms(player: &PlayerSnapshot) -> (u64, Option<u64>) {
    if player.play_started_at.is_none() {
        return (0, None);
    }
    (player.playback_elapsed_ms(), player.play_total_ms)
}

pub(super) fn current_lyric_index(
    lines: &[crate::domain::model::LyricLine],
    elapsed_ms: u64,
) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    match lines.binary_search_by_key(&elapsed_ms, |l| l.time_ms) {
        Ok(i) => Some(i),
        Err(0) => Some(0),
        Err(i) => Some(i - 1),
    }
}

pub(super) fn apply_lyrics_offset(elapsed_ms: u64, offset_ms: i64) -> u64 {
    if offset_ms >= 0 {
        elapsed_ms.saturating_add(offset_ms as u64)
    } else {
        elapsed_ms.saturating_sub((-offset_ms) as u64)
    }
}

pub(super) fn fmt_offset(offset_ms: i64) -> String {
    let sign = if offset_ms < 0 { "-" } else { "+" };
    let abs_ms = offset_ms.unsigned_abs();
    let s = abs_ms as f64 / 1000.0;
    format!("{sign}{s:.2}s")
}

pub(super) fn br_label(br: i64) -> &'static str {
    match br {
        128_000 => "128k",
        192_000 => "192k",
        320_000 => "320k",
        999_000 => "最高",
        _ => "自定义",
    }
}

pub(super) fn play_mode_label(m: PlayMode) -> &'static str {
    match m {
        PlayMode::Sequential => "顺序",
        PlayMode::ListLoop => "列表循环",
        PlayMode::SingleLoop => "单曲循环",
        PlayMode::Shuffle => "随机",
    }
}

pub(super) fn fmt_mmss(ms: u64) -> String {
    let total_sec = ms / 1000;
    let m = total_sec / 60;
    let s = total_sec % 60;
    format!("{m:02}:{s:02}")
}
