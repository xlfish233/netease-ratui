use super::header::draw_header;
use super::layout::{split_body, split_canvas, split_header, split_right};
use super::login_view::draw_login;
use super::lyrics_view::draw_lyrics;
use super::menu::draw_menu_overlay;
use super::overlays::draw_help_overlay;
use super::panels::{draw_context_panel, draw_left_panel, draw_now_panel};
use super::player_status::draw_footer;
use super::playlists_view::draw_playlists;
use super::search_view::draw_search;
use super::settings_view::draw_settings;
use super::toast::draw_toast;
use super::utils::{MIN_CANVAS_HEIGHT, MIN_CANVAS_WIDTH, canvas_rect, is_unauth_login_page};
use crate::app::{AppSnapshot, AppViewSnapshot, UiFocus, View};
use ratatui::{
    Frame,
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(super) fn draw_ui(f: &mut Frame, app: &AppSnapshot) {
    let size = f.area();
    let Some(canvas) = canvas_rect(size) else {
        draw_resize_prompt(f, size);
        return;
    };

    if is_unauth_login_page(app) {
        let canvas_layout = split_canvas(canvas);

        if let AppViewSnapshot::Login(state) = &app.view_state {
            draw_login(f, canvas, state, app.logged_in, true);
        }

        if let Some(toast) = &app.toast {
            draw_toast(f, canvas_layout.toast, toast);
        }

        if app.help_visible {
            draw_help_overlay(f, canvas);
        }

        if app.menu_visible {
            draw_menu_overlay(f, canvas, app);
        }
        return;
    }

    let canvas_layout = split_canvas(canvas);
    let header_layout = split_header(canvas_layout.header);
    let body_layout = split_body(canvas_layout.body);
    let right_layout = split_right(body_layout.right);

    draw_header(f, &header_layout, app);
    draw_left_panel(f, body_layout.left, app);
    draw_context_panel(f, right_layout.context, app);
    draw_now_panel(f, right_layout.now, &app.player, app.ui_focus);

    let center_active = app.ui_focus == UiFocus::BodyCenter;
    match (&app.view, &app.view_state) {
        (View::Login, AppViewSnapshot::Login(state)) => {
            draw_login(f, body_layout.center, state, app.logged_in, false);
        }
        (View::Playlists, AppViewSnapshot::Playlists(state)) => {
            draw_playlists(f, body_layout.center, state, center_active);
        }
        (View::Search, AppViewSnapshot::Search(state)) => {
            draw_search(f, body_layout.center, state, center_active);
        }
        (View::Lyrics, AppViewSnapshot::Lyrics(state)) => {
            draw_lyrics(f, body_layout.center, state, &app.player, center_active);
        }
        (View::Settings, AppViewSnapshot::Settings(state)) => {
            draw_settings(
                f,
                body_layout.center,
                state,
                &app.player,
                app.logged_in,
                center_active,
            );
        }
        _ => {}
    }

    // 绘制 Toast（如果有）
    if let Some(toast) = &app.toast {
        draw_toast(f, canvas_layout.toast, toast);
    }

    let view_status = match &app.view_state {
        AppViewSnapshot::Login(state) => state.login_status.as_str(),
        AppViewSnapshot::Playlists(state) => state.playlists_status.as_str(),
        AppViewSnapshot::Search(state) => state.search_status.as_str(),
        AppViewSnapshot::Lyrics(state) => state.lyrics_status.as_str(),
        AppViewSnapshot::Settings(state) => state.settings_status.as_str(),
    };
    draw_footer(f, canvas_layout.footer, &app.player, view_status);

    if app.help_visible {
        draw_help_overlay(f, canvas);
    }

    if app.menu_visible {
        draw_menu_overlay(f, canvas, app);
    }
}

fn draw_resize_prompt(f: &mut Frame, area: ratatui::layout::Rect) {
    let message = format!(
        "Terminal too small.\nMinimum: {MIN_CANVAS_WIDTH}x{MIN_CANVAS_HEIGHT}\nCurrent: {}x{}\nResize to continue.",
        area.width, area.height
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title("netease-ratui");
    let paragraph = Paragraph::new(Text::from(message))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::draw_ui;
    use crate::app::{App, AppSnapshot};

    fn render_to_string(snapshot: &AppSnapshot) -> String {
        let backend = ratatui::backend::TestBackend::new(122, 29);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal.draw(|f| draw_ui(f, snapshot)).expect("draw");

        let backend = terminal.backend();
        let buffer = backend.buffer();
        let width = buffer.area.width as usize;
        buffer
            .content()
            .chunks(width)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn unauth_login_page_uses_full_canvas_without_header() {
        let app = App {
            login_qr_url: Some("https://music.163.com/login?codekey=test".to_owned()),
            login_qr_ascii: Some("██\n██".to_owned()),
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);

        let rendered = render_to_string(&snapshot);
        assert!(rendered.contains("URL:"));
        assert!(
            !rendered.contains("Search[1]:"),
            "未登录专页不应渲染常规 header"
        );
    }

    #[test]
    fn unauth_cookie_login_page_renders_music_u_input() {
        let app = App {
            login_cookie_input_visible: true,
            login_cookie_input: "cookie-value".to_owned(),
            ..Default::default()
        };
        let snapshot = AppSnapshot::from_app(&app);

        let rendered = render_to_string(&snapshot);
        assert!(rendered.contains("MUSIC_U"));
        assert!(rendered.contains("cookie-value"));
        assert!(
            !rendered.contains("Search[1]:"),
            "未登录专页不应渲染常规 header"
        );
    }
}
