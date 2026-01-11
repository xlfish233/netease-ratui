use crate::app::{App, PlaylistMode, View};
use crate::messages::app::{AppCommand, AppEvent};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
};
use std::io;
use std::io::Write;
use std::time::{Duration, Instant};
use tokio::sync::mpsc as tokio_mpsc;

struct TuiGuard;

impl TuiGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        stdout.flush()?;
        Ok(Self)
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, cursor::Show, LeaveAlternateScreen);
        let _ = stdout.flush();
    }
}

pub async fn run_tui(
    mut app: App,
    tx: tokio_mpsc::Sender<AppCommand>,
    mut rx: tokio_mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    let _guard = TuiGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let _ = tx.send(AppCommand::Bootstrap).await;

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        while let Ok(evt) = rx.try_recv() {
            match evt {
                AppEvent::State(s) => app = s,
                AppEvent::Toast(s) => match app.view {
                    View::Login => app.login_status = s,
                    View::Playlists => app.playlists_status = s,
                    View::Search => app.search_status = s,
                    View::Lyrics => app.lyrics_status = s,
                },
                AppEvent::Error(e) => match app.view {
                    View::Login => app.login_status = format!("错误: {e}"),
                    View::Playlists => app.playlists_status = format!("错误: {e}"),
                    View::Search => app.search_status = format!("错误: {e}"),
                    View::Lyrics => app.lyrics_status = format!("错误: {e}"),
                },
            }
        }

        terminal.draw(|f| draw_ui(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key(&app, key, &tx).await {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

async fn handle_key(app: &App, key: KeyEvent, tx: &tokio_mpsc::Sender<AppCommand>) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        } => {
            let _ = tx.send(AppCommand::Quit).await;
            return true;
        }
        KeyEvent {
            code: KeyCode::Tab, ..
        } => {
            let _ = tx.send(AppCommand::TabNext).await;
        }
        _ => {}
    }

    // Global player controls (avoid interfering with text input as much as possible)
    match (key.code, key.modifiers) {
        (KeyCode::Char(' '), _) => {
            let _ = tx.send(AppCommand::PlayerTogglePause).await;
            return false;
        }
        (KeyCode::Char('['), _) => {
            let _ = tx.send(AppCommand::PlayerPrev).await;
            return false;
        }
        (KeyCode::Char(']'), _) => {
            let _ = tx.send(AppCommand::PlayerNext).await;
            return false;
        }
        (KeyCode::Char('M'), _) => {
            let _ = tx.send(AppCommand::PlayerCycleMode).await;
            return false;
        }
        (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx.send(AppCommand::PlayerStop).await;
            return false;
        }
        (KeyCode::Left, m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx
                .send(AppCommand::PlayerSeekBackwardMs { ms: 5_000 })
                .await;
            return false;
        }
        (KeyCode::Right, m) if m.contains(KeyModifiers::CONTROL) => {
            let _ = tx.send(AppCommand::PlayerSeekForwardMs { ms: 5_000 }).await;
            return false;
        }
        (KeyCode::Up, m) if m.contains(KeyModifiers::ALT) => {
            let _ = tx.send(AppCommand::PlayerVolumeUp).await;
            return false;
        }
        (KeyCode::Down, m) if m.contains(KeyModifiers::ALT) => {
            let _ = tx.send(AppCommand::PlayerVolumeDown).await;
            return false;
        }
        (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) && matches!(app.view, View::Lyrics) => {
            let ms = if m.contains(KeyModifiers::SHIFT) { -50 } else { -200 };
            let _ = tx.send(AppCommand::LyricsOffsetAddMs { ms }).await;
            return false;
        }
        (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) && matches!(app.view, View::Lyrics) => {
            let ms = if m.contains(KeyModifiers::SHIFT) { 50 } else { 200 };
            let _ = tx.send(AppCommand::LyricsOffsetAddMs { ms }).await;
            return false;
        }
        _ => {}
    }

    match app.view {
        View::Login => {
            if let KeyCode::Char('l') = key.code {
                let _ = tx.send(AppCommand::LoginGenerateQr).await;
            }
        }
        View::Playlists => match key.code {
            KeyCode::Char('b') => {
                let _ = tx.send(AppCommand::Back).await;
            }
            KeyCode::Enter => {
                let _ = tx.send(AppCommand::PlaylistsOpenSelected).await;
            }
            KeyCode::Char('p') => {
                let _ = tx.send(AppCommand::PlaylistTracksPlaySelected).await;
            }
            KeyCode::Up => match app.playlist_mode {
                PlaylistMode::List => {
                    let _ = tx.send(AppCommand::PlaylistsMoveUp).await;
                }
                PlaylistMode::Tracks => {
                    let _ = tx.send(AppCommand::PlaylistTracksMoveUp).await;
                }
            },
            KeyCode::Down => match app.playlist_mode {
                PlaylistMode::List => {
                    let _ = tx.send(AppCommand::PlaylistsMoveDown).await;
                }
                PlaylistMode::Tracks => {
                    let _ = tx.send(AppCommand::PlaylistTracksMoveDown).await;
                }
            },
            _ => {}
        },
        View::Search => match key.code {
            KeyCode::Char('p') => {
                let _ = tx.send(AppCommand::SearchPlaySelected).await;
            }
            KeyCode::Enter => {
                let _ = tx.send(AppCommand::SearchSubmit).await;
            }
            KeyCode::Backspace => {
                let _ = tx.send(AppCommand::SearchInputBackspace).await;
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    let _ = tx.send(AppCommand::SearchInputChar { c }).await;
                }
            }
            KeyCode::Up => {
                let _ = tx.send(AppCommand::SearchMoveUp).await;
            }
            KeyCode::Down => {
                let _ = tx.send(AppCommand::SearchMoveDown).await;
            }
            _ => {}
        },
        View::Lyrics => match key.code {
            KeyCode::Char('o') => {
                let _ = tx.send(AppCommand::LyricsToggleFollow).await;
            }
            KeyCode::Char('g') => {
                let _ = tx.send(AppCommand::LyricsGotoCurrent).await;
            }
            KeyCode::Up => {
                let _ = tx.send(AppCommand::LyricsMoveUp).await;
            }
            KeyCode::Down => {
                let _ = tx.send(AppCommand::LyricsMoveDown).await;
            }
            _ => {}
        },
    }

    false
}

fn draw_ui(f: &mut ratatui::Frame, app: &App) {
    let size = f.area();

    let (titles, selected) = if app.logged_in {
        (
            ["歌单", "搜索", "歌词"]
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
            match app.view {
                View::Playlists => 0,
                View::Search => 1,
                View::Lyrics => 2,
                View::Login => 0,
            },
        )
    } else {
        (
            ["登录", "搜索", "歌词"]
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
            match app.view {
                View::Login => 0,
                View::Search => 1,
                View::Lyrics => 2,
                View::Playlists => 1,
            },
        )
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("netease-ratui"),
        )
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Yellow));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(size);
    f.render_widget(tabs, chunks[0]);

    match app.view {
        View::Login => draw_login(f, chunks[1], app),
        View::Playlists => draw_playlists(f, chunks[1], app),
        View::Search => draw_search(f, chunks[1], app),
        View::Lyrics => draw_lyrics(f, chunks[1], app),
    }
}

fn draw_playlists(f: &mut ratatui::Frame, area: ratatui::prelude::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(7)])
        .split(area);

    let title = match app.playlist_mode {
        PlaylistMode::List => "歌单(↑↓选择 回车打开)",
        PlaylistMode::Tracks => "歌曲(↑↓选择 p 播放 b 返回)",
    };
    let items: Vec<ListItem> = match app.playlist_mode {
        PlaylistMode::List => app
            .playlists
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mark = if p.special_type == 5 || p.name.contains("我喜欢") {
                    " ♥"
                } else {
                    ""
                };
                ListItem::new(Line::from(format!(
                    "{}  {} ({}首){}",
                    i + 1,
                    p.name,
                    p.track_count,
                    mark
                )))
            })
            .collect(),
        PlaylistMode::Tracks => app
            .playlist_tracks
            .iter()
            .enumerate()
            .map(|(i, s)| {
                ListItem::new(Line::from(format!("{}  {} - {}", i + 1, s.name, s.artists)))
            })
            .collect(),
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().fg(Color::Yellow));

    let mut st = ratatui::widgets::ListState::default();
    let sel = match app.playlist_mode {
        PlaylistMode::List => app.playlists_selected,
        PlaylistMode::Tracks => app.playlist_tracks_selected,
    };
    st.select(Some(sel));
    f.render_stateful_widget(list, chunks[0], &mut st);

    draw_player_status(
        f,
        chunks[1],
        app,
        "状态",
        "歌单",
        app.playlists_status.as_str(),
    );
}

fn draw_login(f: &mut ratatui::Frame, area: ratatui::prelude::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(5)])
        .split(area);

    let qr = app.login_qr_ascii.as_deref().unwrap_or("按 l 生成二维码");
    let qr_block = Paragraph::new(Text::from(qr))
        .block(Block::default().borders(Borders::ALL).title("二维码"))
        .wrap(Wrap { trim: false });
    f.render_widget(qr_block, chunks[0]);

    let info = format!(
        "状态: {}\n已登录: {}\nURL: {}\n操作: l 生成二维码 | Tab 切换 | q 退出",
        app.login_status,
        if app.logged_in { "是" } else { "否" },
        app.login_qr_url.as_deref().unwrap_or("-")
    );
    let info_block =
        Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("信息"));
    f.render_widget(info_block, chunks[1]);
}

fn draw_search(f: &mut ratatui::Frame, area: ratatui::prelude::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(area);

    let input = Paragraph::new(app.search_input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("关键词(回车搜索)"),
    );
    f.render_widget(input, chunks[0]);

    let items = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let line = format!("{}  {} - {}  ({})", s.id, s.name, s.artists, i + 1);
            ListItem::new(Line::from(line))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("结果(↑↓选择)"))
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_stateful_widget(list, chunks[1], &mut list_state(app.search_selected));

    draw_player_status(
        f,
        chunks[2],
        app,
        "状态",
        "搜索",
        app.search_status.as_str(),
    );
}

fn draw_lyrics(f: &mut ratatui::Frame, area: ratatui::prelude::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(7)])
        .split(area);

    let offset_text = fmt_offset(app.lyrics_offset_ms);
    let mode_text = if app.lyrics_follow { "跟随" } else { "锁定" };
    let status_text = format!("{} | {} | offset={}", app.lyrics_status, mode_text, offset_text);

    if app.lyrics.is_empty() {
        let block = Paragraph::new(app.lyrics_status.as_str())
            .block(Block::default().borders(Borders::ALL).title("歌词"))
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
    } else {
        let (elapsed_ms, _) = playback_time_ms(app);
        let selected = if app.lyrics_follow {
            current_lyric_index(&app.lyrics, apply_lyrics_offset(elapsed_ms, app.lyrics_offset_ms))
                .unwrap_or(0)
        } else {
            app.lyrics_selected.min(app.lyrics.len().saturating_sub(1))
        };

        let items = app
            .lyrics
            .iter()
            .map(|l| {
                let mut lines = vec![Line::from(l.text.as_str())];
                if let Some(t) = l.translation.as_deref() {
                    if !t.trim().is_empty() {
                        lines.push(Line::from(format!("  {t}")));
                    }
                }
                ListItem::new(Text::from(lines))
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("歌词（自动滚动）"))
            .highlight_style(Style::default().fg(Color::Yellow));
        f.render_stateful_widget(list, chunks[0], &mut list_state(selected));
    }

    draw_player_status(f, chunks[1], app, "状态", "歌词", status_text.as_str());
}

fn list_state(selected: usize) -> ratatui::widgets::ListState {
    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(selected));
    st
}

fn selected_song_title(app: &App) -> String {
    app.search_results
        .get(app.search_selected)
        .map(|s| format!("{} - {}", s.name, s.artists))
        .unwrap_or_else(|| "未知歌曲".to_owned())
}

fn playback_time_ms(app: &App) -> (u64, Option<u64>) {
    let Some(started) = app.play_started_at else {
        return (0, None);
    };

    let now = if app.paused {
        app.play_paused_at.unwrap_or_else(Instant::now)
    } else {
        Instant::now()
    };

    let elapsed = now
        .duration_since(started)
        .as_millis()
        .saturating_sub(app.play_paused_accum_ms as u128) as u64;
    (elapsed, app.play_total_ms)
}

fn current_lyric_index(lines: &[crate::domain::model::LyricLine], elapsed_ms: u64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    match lines.binary_search_by_key(&elapsed_ms, |l| l.time_ms) {
        Ok(i) => Some(i),
        Err(0) => Some(0),
        Err(i) => Some(i - 1),
    }
}

fn apply_lyrics_offset(elapsed_ms: u64, offset_ms: i64) -> u64 {
    if offset_ms >= 0 {
        elapsed_ms.saturating_add(offset_ms as u64)
    } else {
        elapsed_ms.saturating_sub((-offset_ms) as u64)
    }
}

fn fmt_offset(offset_ms: i64) -> String {
    let sign = if offset_ms < 0 { "-" } else { "+" };
    let abs_ms = offset_ms.unsigned_abs();
    let s = abs_ms as f64 / 1000.0;
    format!("{sign}{s:.2}s")
}

fn fmt_mmss(ms: u64) -> String {
    let total_sec = ms / 1000;
    let m = total_sec / 60;
    let s = total_sec % 60;
    format!("{m:02}:{s:02}")
}

fn draw_player_status(
    f: &mut ratatui::Frame,
    area: ratatui::prelude::Rect,
    app: &App,
    title: &str,
    context_label: &str,
    context_value: &str,
) {
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(3)])
        .split(area);

    let now = app.now_playing.as_deref().unwrap_or("-");
    let (elapsed_ms, total_ms) = playback_time_ms(app);
    let progress = progress_bar_text(elapsed_ms, total_ms, 24);
    let time_text = format!(
        "{} / {}{}",
        fmt_mmss(elapsed_ms),
        total_ms.map(fmt_mmss).unwrap_or_else(|| "--:--".to_owned()),
        if app.paused { " (暂停)" } else { "" }
    );
    let mode_text = match app.play_mode {
        crate::app::PlayMode::Sequential => "顺序",
        crate::app::PlayMode::ListLoop => "列表循环",
        crate::app::PlayMode::SingleLoop => "单曲循环",
        crate::app::PlayMode::Shuffle => "随机",
    };

    let status = Paragraph::new(format!(
        "{}: {}\n播放: {} | Now: {}\n时间: {} | 模式: {} | 音量: {:.0}%\n{}",
        context_label,
        context_value,
        app.play_status,
        now,
        time_text,
        mode_text,
        (app.volume.clamp(0.0, 1.0) * 100.0),
        progress
    ))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(status, status_chunks[0]);

    // 底栏帮助提示
    let help = Paragraph::new(
        "帮助: Tab 切换页 | ↑↓ 选择/滚动 | Enter 打开歌单 | p 播放选中 | 空格 暂停/继续 | Ctrl+S 停止 | [/] 上一首/下一首 | Ctrl+←/→ Seek | Alt+↑/↓ 音量 | M 切换模式 | 歌词页: o 跟随/锁定 | g 回到当前行 | Alt+←/→ offset(±200ms) | Shift+Alt+←/→ offset(±50ms) | q 退出",
    )
    .block(Block::default().borders(Borders::ALL).title("帮助"));
    f.render_widget(help, status_chunks[1]);
}

fn progress_bar_text(elapsed_ms: u64, total_ms: Option<u64>, width: usize) -> String {
    let Some(total_ms) = total_ms.filter(|t| *t > 0) else {
        return "进度: [------------------------]".to_owned();
    };

    let ratio = (elapsed_ms.min(total_ms) as f64) / (total_ms as f64);
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let bar = "#".repeat(filled) + &"-".repeat(width - filled);
    format!("进度: [{bar}]")
}
