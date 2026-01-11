use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(super) fn draw_login(f: &mut Frame, area: Rect, app: &App) {
    if app.login_cookie_input_visible {
        // Cookie 输入模式
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // 提示信息
                Constraint::Length(3), // 输入框
                Constraint::Min(5),    // 说明区域
            ])
            .split(area);

        let hint = Paragraph::new("手动登录：输入 MUSIC_U Cookie 值")
            .block(Block::default().borders(Borders::ALL).title("提示"));
        f.render_widget(hint, chunks[0]);

        let input = Paragraph::new(app.login_cookie_input.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("MUSIC_U (回车提交，Esc 取消)"),
        );
        f.render_widget(input, chunks[1]);

        let help = "获取方式：\n\
            1. 浏览器登录 music.163.com\n\
            2. 开发者工具(F12) -> Application -> Cookies\n\
            3. 找到 MUSIC_U 并复制值\n\
            \n\
            快捷键: Enter 提交 | Esc 取消 | c 二维码登录";
        let help_block =
            Paragraph::new(help).block(Block::default().borders(Borders::ALL).title("帮助"));
        f.render_widget(help_block, chunks[2]);
    } else {
        // 二维码登录模式
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(9)])
            .split(area);

        let qr_hint = if app.login_qr_ascii.is_some() {
            ""
        } else {
            "\n\n按 l 生成二维码，或按 c 使用 Cookie 登录"
        };
        let qr_display = format!(
            "{}{}",
            app.login_qr_ascii.as_deref().unwrap_or("尚未生成二维码"),
            qr_hint
        );
        let qr_block = Paragraph::new(Text::from(qr_display))
            .block(Block::default().borders(Borders::ALL).title("二维码登录"))
            .wrap(Wrap { trim: false });
        f.render_widget(qr_block, chunks[0]);

        let info = format!(
            "状态: {}\n\
            已登录: {}\n\
            URL: {}\n\
            \n\
            快捷键:\n\
            l - 生成二维码 | c - Cookie 登录\n\
            Tab - 切换页面 | q - 退出\n\
            \n\
            Cookie 登录：浏览器登录 music.163.com\n\
            后按 c，输入 MUSIC_U 值即可",
            app.login_status,
            if app.logged_in {
                "是"
            } else {
                "否 (可扫码或 Cookie 登录)"
            },
            app.login_qr_url.as_deref().unwrap_or("-")
        );
        let info_block =
            Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("操作说明"));
        f.render_widget(info_block, chunks[1]);
    }
}
