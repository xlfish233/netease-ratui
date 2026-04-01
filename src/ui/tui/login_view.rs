use crate::app::LoginSnapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(super) fn draw_login(
    f: &mut Frame,
    area: Rect,
    state: &LoginSnapshot,
    logged_in: bool,
    full_page: bool,
) {
    if full_page {
        draw_login_full_page(f, area, state, logged_in);
        return;
    }

    draw_login_compact(f, area, state, logged_in);
}

fn draw_login_full_page(f: &mut Frame, area: Rect, state: &LoginSnapshot, logged_in: bool) {
    if state.login_cookie_input_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(10),
            ])
            .split(area);

        let hint = format!(
            "状态: {}\n手动登录：输入 MUSIC_U Cookie 值",
            state.login_status
        );
        let hint_block = Paragraph::new(hint)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Cookie 登录[3]"),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(hint_block, chunks[0]);

        let input = Paragraph::new(state.login_cookie_input.as_str()).block(
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
            快捷键：Enter 提交 | Esc 取消 | c 二维码登录\n\
            F1-F4 / Ctrl+Tab 切换页面 | ? 帮助 | q 退出";
        let help_block = Paragraph::new(help)
            .block(Block::default().borders(Borders::ALL).title("帮助"))
            .wrap(Wrap { trim: false });
        f.render_widget(help_block, chunks[2]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(74), Constraint::Min(32)])
        .split(area);

    let qr_hint = if state.login_qr_ascii.is_some() {
        ""
    } else {
        "\n\n按 l 生成二维码\n按 c 使用 Cookie 登录"
    };
    let qr_display = format!(
        "{}{}",
        state.login_qr_ascii.as_deref().unwrap_or("尚未生成二维码"),
        qr_hint
    );
    let qr_block = Paragraph::new(Text::from(qr_display))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("二维码登录[3]"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(qr_block, chunks[0]);

    let info = format!(
        "状态:\n{}\n\n已登录: {}\n\n快捷键:\n\
        l - 生成二维码\n\
        c - Cookie 登录\n\
        F1-F4 / Ctrl+Tab - 切换页面\n\
        ? - 帮助\n\
        q - 退出\n\n\
        Cookie 登录：\n浏览器登录 music.163.com\n后按 c 输入 MUSIC_U\n\n\
        URL:\n{}",
        state.login_status,
        if logged_in {
            "是"
        } else {
            "否 (可扫码或 Cookie 登录)"
        },
        state.login_qr_url.as_deref().unwrap_or("-")
    );
    let info_block = Paragraph::new(info)
        .block(Block::default().borders(Borders::ALL).title("操作说明"))
        .wrap(Wrap { trim: false });
    f.render_widget(info_block, chunks[1]);
}

fn draw_login_compact(f: &mut Frame, area: Rect, state: &LoginSnapshot, logged_in: bool) {
    if state.login_cookie_input_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(5),
            ])
            .split(area);

        let hint = Paragraph::new("手动登录：输入 MUSIC_U Cookie 值")
            .block(Block::default().borders(Borders::ALL).title("提示"));
        f.render_widget(hint, chunks[0]);

        let input = Paragraph::new(state.login_cookie_input.as_str()).block(
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
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(9)])
        .split(area);

    let qr_hint = if state.login_qr_ascii.is_some() {
        ""
    } else {
        "\n\n按 l 生成二维码，或按 c 使用 Cookie 登录"
    };
    let qr_display = format!(
        "{}{}",
        state.login_qr_ascii.as_deref().unwrap_or("尚未生成二维码"),
        qr_hint
    );
    let qr_block = Paragraph::new(Text::from(qr_display))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("二维码登录[3]"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(qr_block, chunks[0]);

    let info = format!(
        "状态: {}\n\
        已登录: {}\n\
        URL: {}\n\
        \n\
        快捷键:\n\
        l - 生成二维码 | c - Cookie 登录\n\
        Ctrl+Tab - 切换页面 | q - 退出\n\
        \n\
        Cookie 登录：浏览器登录 music.163.com\n\
        后按 c，输入 MUSIC_U 值即可",
        state.login_status,
        if logged_in {
            "是"
        } else {
            "否 (可扫码或 Cookie 登录)"
        },
        state.login_qr_url.as_deref().unwrap_or("-")
    );
    let info_block =
        Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("操作说明[3]"));
    f.render_widget(info_block, chunks[1]);
}
