use super::playlists_view::draw_playlist_list;
use super::styles::focus_style;
use super::utils::{
    apply_lyrics_offset, br_label, current_lyric_index, fmt_offset, playback_time_ms,
};
use crate::app::{
    AppSnapshot, AppViewSnapshot, PlayerSnapshot, UiFocus, tab_configs, tab_index_for_view,
};
use ratatui::{
    Frame,
    prelude::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};

pub(super) fn draw_left_panel(f: &mut Frame, area: Rect, app: &AppSnapshot) {
    match &app.view_state {
        AppViewSnapshot::Playlists(state) => {
            draw_playlist_list(f, area, state, app.ui_focus == UiFocus::BodyLeft);
        }
        AppViewSnapshot::Search(state) => {
            draw_left_info(
                f,
                area,
                "搜索",
                vec![
                    Line::from(format!("关键词: {}", app.search_input)),
                    Line::from(state.search_status.as_str()),
                    Line::from(format!("结果: {}", state.search_results.len())),
                ],
                app.ui_focus == UiFocus::BodyLeft,
            );
        }
        AppViewSnapshot::Lyrics(state) => {
            draw_left_info(
                f,
                area,
                "歌词",
                vec![
                    Line::from(format!(
                        "模式: {}",
                        if state.lyrics_follow {
                            "跟随"
                        } else {
                            "锁定"
                        }
                    )),
                    Line::from(format!("offset: {}", fmt_offset(state.lyrics_offset_ms))),
                    Line::from(format!("行数: {}", state.lyrics.len())),
                ],
                app.ui_focus == UiFocus::BodyLeft,
            );
        }
        AppViewSnapshot::Settings(state) => {
            let categories = vec![("播放", 0), ("歌词", 1), ("缓存", 2), ("账号", 3)];
            let lines: Vec<Line> = categories
                .into_iter()
                .map(|(label, idx)| {
                    let mark = if idx == state.settings_group_selected {
                        ">"
                    } else {
                        " "
                    };
                    Line::from(format!("{mark}{label}"))
                })
                .collect();
            draw_left_info(
                f,
                area,
                "设置分组",
                lines,
                app.ui_focus == UiFocus::BodyLeft,
            );
        }
        AppViewSnapshot::Login(state) => {
            draw_left_info(
                f,
                area,
                "登录",
                vec![
                    Line::from(state.login_status.as_str()),
                    Line::from("l 生成二维码"),
                    Line::from("c Cookie 登录"),
                ],
                app.ui_focus == UiFocus::BodyLeft,
            );
        }
    }
}

fn draw_left_info(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, active: bool) {
    let style = focus_style(active);
    let panel = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{}[2]", title))
                .style(style),
        )
        .style(style);
    f.render_widget(panel, area);
}

#[allow(dead_code)]
pub(super) fn draw_nav_panel(f: &mut Frame, area: Rect, app: &AppSnapshot) {
    let configs = tab_configs(app.logged_in);
    let selected = tab_index_for_view(app.view, app.logged_in).unwrap_or(0);
    let lines: Vec<Line> = configs
        .iter()
        .enumerate()
        .map(|(i, cfg)| {
            if i == selected {
                Line::from(format!("> {}", cfg.title))
            } else {
                Line::from(format!("  {}", cfg.title))
            }
        })
        .collect();

    let style = focus_style(app.ui_focus == UiFocus::BodyLeft);
    let panel = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("导航")
                .style(style),
        )
        .style(style);
    f.render_widget(panel, area);
}

pub(super) fn draw_now_panel(f: &mut Frame, area: Rect, player: &PlayerSnapshot, focus: UiFocus) {
    let now = player.now_playing.as_deref().unwrap_or("-");
    let mode = match player.play_mode {
        crate::app::PlayMode::Sequential => "顺序",
        crate::app::PlayMode::ListLoop => "列表循环",
        crate::app::PlayMode::SingleLoop => "单曲循环",
        crate::app::PlayMode::Shuffle => "随机",
    };
    let lines = vec![
        Line::from(format!("Now:{now}")),
        Line::from(format!("状态:{}", player.play_status)),
        Line::from(format!("模式:{mode}")),
        Line::from(format!("音量:{:.0}%", player.volume * 100.0)),
        Line::from(format!("音质:{}", br_label(player.play_br))),
    ];

    let style = focus_style(focus == UiFocus::BodyRight);
    let panel = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Now[4]")
                .style(style),
        )
        .style(style);
    f.render_widget(panel, area);
}

pub(super) fn draw_context_panel(f: &mut Frame, area: Rect, app: &AppSnapshot) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let (title, lines) = match &app.view_state {
        AppViewSnapshot::Login(state) => (
            "登录",
            vec![
                Line::from(state.login_status.as_str()),
                Line::from(format!(
                    "已登录: {}",
                    if app.logged_in { "是" } else { "否" }
                )),
            ],
        ),
        AppViewSnapshot::Playlists(state) => {
            let (mode, total, selected) = match state.playlist_mode {
                crate::app::PlaylistMode::List => {
                    ("歌单", state.playlists.len(), state.playlists_selected)
                }
                crate::app::PlaylistMode::Tracks => (
                    "歌曲",
                    state.playlist_tracks.len(),
                    state.playlist_tracks_selected,
                ),
            };
            let mut lines = vec![
                Line::from(state.playlists_status.as_str()),
                Line::from(format!("模式: {mode}")),
                Line::from(format!(
                    "数量: {total} | 选中: {}",
                    if total == 0 { 0 } else { selected + 1 }
                )),
            ];
            if matches!(state.playlist_mode, crate::app::PlaylistMode::List) {
                if let Some(p) = state.playlists.get(state.playlists_selected) {
                    lines.push(Line::from(format!("歌单: {}", p.name)));
                    lines.push(Line::from(format!("曲目: {}", p.track_count)));
                }
            } else if let Some(s) = state.playlist_tracks.get(state.playlist_tracks_selected) {
                lines.push(Line::from(format!("歌曲: {}", s.name)));
                lines.push(Line::from(format!("艺人: {}", s.artists)));
            }
            let queue_max_lines = inner_height.saturating_sub(lines.len());
            lines.extend(queue_preview_lines(app, queue_max_lines));
            ("歌单", lines)
        }
        AppViewSnapshot::Search(state) => {
            let mut lines = vec![
                Line::from(format!("关键词: {}", app.search_input)),
                Line::from(state.search_status.as_str()),
                Line::from(format!("结果: {}", state.search_results.len())),
                Line::from(format!(
                    "选中: {}",
                    if state.search_results.is_empty() {
                        0
                    } else {
                        state.search_selected + 1
                    }
                )),
            ];
            if let Some(s) = state.search_results.get(state.search_selected) {
                lines.push(Line::from(format!("歌曲: {}", s.name)));
                lines.push(Line::from(format!("艺人: {}", s.artists)));
            }
            let queue_max_lines = inner_height.saturating_sub(lines.len());
            lines.extend(queue_preview_lines(app, queue_max_lines));
            ("搜索", lines)
        }
        AppViewSnapshot::Lyrics(state) => {
            let mut lines = vec![
                Line::from(state.lyrics_status.as_str()),
                Line::from(format!(
                    "模式: {}",
                    if state.lyrics_follow {
                        "跟随"
                    } else {
                        "锁定"
                    }
                )),
                Line::from(format!("offset: {}", fmt_offset(state.lyrics_offset_ms))),
                Line::from(format!("行数: {}", state.lyrics.len())),
            ];
            if !state.lyrics.is_empty() {
                let (elapsed_ms, _) = playback_time_ms(&app.player);
                let idx = current_lyric_index(
                    &state.lyrics,
                    apply_lyrics_offset(elapsed_ms, state.lyrics_offset_ms),
                )
                .unwrap_or(0);
                if let Some(line) = state.lyrics.get(idx) {
                    lines.push(Line::from(format!("当前: {}", line.text)));
                }
            }
            ("歌词", lines)
        }
        AppViewSnapshot::Settings(state) => (
            "设置",
            vec![
                Line::from(state.settings_status.as_str()),
                Line::from(format!("选中: {}", state.settings_selected + 1)),
                Line::from(format!("音质: {}", br_label(app.player.play_br))),
                Line::from(format!("音量: {:.0}%", app.player.volume * 100.0)),
                Line::from(format!(
                    "模式: {}",
                    match app.player.play_mode {
                        crate::app::PlayMode::Sequential => "顺序",
                        crate::app::PlayMode::ListLoop => "列表循环",
                        crate::app::PlayMode::SingleLoop => "单曲循环",
                        crate::app::PlayMode::Shuffle => "随机",
                    }
                )),
                Line::from(format!("offset: {}", fmt_offset(state.lyrics_offset_ms))),
                Line::from(format!("淡入淡出: {}ms", state.crossfade_ms)),
            ],
        ),
    };

    let style = focus_style(app.ui_focus == UiFocus::BodyRight);
    let panel = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{}[4]", title))
                .style(style),
        )
        .style(style);
    f.render_widget(panel, area);
}

fn queue_preview_lines(app: &AppSnapshot, max_lines: usize) -> Vec<Line<'_>> {
    if max_lines == 0 {
        return Vec::new();
    }
    if app.queue.is_empty() {
        return vec![Line::from("队列: 空")];
    }

    let total = app.queue.len();
    let start = app.queue_pos.unwrap_or(0).min(total.saturating_sub(1));
    let mut lines = Vec::with_capacity(max_lines);
    lines.push(Line::from(format!("队列:{}/{}", start + 1, total)));
    let max_songs = max_lines.saturating_sub(1);
    for (i, song) in app.queue.iter().skip(start).take(max_songs).enumerate() {
        let idx = start + i + 1;
        let marker = if i == 0 { ">" } else { " " };
        lines.push(Line::from(format!(
            "{marker}{}.{}-{}",
            idx, song.name, song.artists
        )));
    }
    lines
}
