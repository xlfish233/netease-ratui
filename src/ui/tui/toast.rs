use crate::app::{Toast, ToastLevel};
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// 绘制 Toast 通知
pub fn draw_toast(f: &mut Frame, area: Rect, toast: &Toast) {
    let (icon, color, close_hint) = match toast.level {
        ToastLevel::Error => ("❌", Color::Red, "[Esc 关闭]"),
        ToastLevel::Warning => ("⚠️ ", Color::Yellow, "[Esc 关闭]"),
        ToastLevel::Info => ("ℹ️ ", Color::Gray, ""),
    };

    let paragraph = Paragraph::new(format!(
        "{} {}  {}",
        icon, &toast.message, close_hint
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(color)),
    )
    .wrap(Wrap { trim: true })
    .style(Style::default().fg(color));

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_error_duration() {
        assert_eq!(ToastLevel::Error.duration_ms(), None);
    }

    #[test]
    fn test_toast_warning_duration() {
        assert_eq!(ToastLevel::Warning.duration_ms(), Some(5000));
    }

    #[test]
    fn test_toast_info_duration() {
        assert_eq!(ToastLevel::Info.duration_ms(), Some(3000));
    }

    #[test]
    fn test_toast_expiration() {
        let toast = Toast::info("test");
        assert!(!toast.is_expired());

        let toast = Toast::error("test");
        assert!(!toast.is_expired()); // Error 永不过期
    }
}
