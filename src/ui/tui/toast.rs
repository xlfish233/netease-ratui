use crate::app::{Toast, ToastLevel};
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// 绘制 Toast 通知
pub fn draw_toast(f: &mut Frame, area: Rect, toast: &Toast) {
    let (icon, color) = match toast.level {
        ToastLevel::Error => ("❌", Color::Red),
        ToastLevel::Warning => ("⚠️ ", Color::Yellow),
        ToastLevel::Info => ("ℹ️ ", Color::Gray),
    };

    let paragraph = Paragraph::new(format!("{} {}", icon, &toast.message))
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
        assert_eq!(ToastLevel::Error.duration_ms(), Some(8000));
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
        assert!(!toast.is_expired()); // Error 8秒后才过期
    }

    /// VAL-TOAST-004: Error Toast 8秒自动消失
    #[test]
    fn test_error_toast_has_8s_duration() {
        let toast = Toast::error("test error");
        assert_eq!(toast.level, ToastLevel::Error);
        assert_eq!(toast.level.duration_ms(), Some(8000));
    }

    /// VAL-TOAST-005: Info Toast 3秒自动消失
    #[test]
    fn test_info_toast_has_3s_duration() {
        let toast = Toast::info("test info");
        assert_eq!(toast.level, ToastLevel::Info);
        assert_eq!(toast.level.duration_ms(), Some(3000));
    }

    /// VAL-TOAST-006: Warning Toast 5秒自动消失
    #[test]
    fn test_warning_toast_has_5s_duration() {
        let toast = Toast::warning("test warning");
        assert_eq!(toast.level, ToastLevel::Warning);
        assert_eq!(toast.level.duration_ms(), Some(5000));
    }
}
