use crate::api_worker::{ApiEvent, ApiRequest};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::app::{parse_search_songs, parse_user_playlists, App, PlaylistMode, View};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;

pub async fn run_tui(
    mut app: App,
    tx: tokio_mpsc::Sender<ApiRequest>,
    mut rx: tokio_mpsc::Receiver<ApiEvent>,
) -> io::Result<()> {
    let (tx_audio, rx_audio) = crate::audio_worker::spawn_audio_worker();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();
    let mut last_qr_poll = Instant::now() - Duration::from_secs(10);

    loop {
        while let Ok(evt) = rx.try_recv() {
            handle_api_event(&mut app, evt, &tx, &tx_audio).await;
        }
        while let Ok(evt) = rx_audio.try_recv() {
            if let Some(req) = handle_audio_event(&mut app, evt) {
                let _ = tx.send(req).await;
            }
        }

    if app.login_unikey.is_some() && !app.logged_in && last_qr_poll.elapsed() >= Duration::from_secs(2) {
        if let Some(key) = app.login_unikey.clone() {
            let _ = tx.send(ApiRequest::LoginQrCheck { key }).await;
            last_qr_poll = Instant::now();
        }
    }

    terminal.draw(|f| draw_ui(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key(&mut app, key, &tx, &tx_audio).await {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

async fn handle_api_event(
    app: &mut App,
    evt: ApiEvent,
    tx: &tokio_mpsc::Sender<ApiRequest>,
    tx_audio: &mpsc::Sender<AudioCommand>,
) {
    match evt {
        ApiEvent::Info(s) => match app.view {
            View::Login => app.login_status = s,
            View::Playlists => app.playlists_status = s,
            View::Search => app.search_status = s,
        },
        ApiEvent::Error(e) => match app.view {
            View::Login => app.login_status = format!("错误: {e}"),
            View::Playlists => app.playlists_status = format!("错误: {e}"),
            View::Search => app.search_status = format!("错误: {e}"),
        },
        ApiEvent::ClientReady { logged_in } => {
            app.logged_in = logged_in;
            if app.logged_in {
                app.view = View::Playlists;
                app.playlists_status = "已登录（已从本地状态恢复），正在加载账号信息...".to_owned();
                let _ = tx.send(ApiRequest::Account).await;
            }
        }
        ApiEvent::LoginQrReady { unikey, url, ascii } => {
            app.login_unikey = Some(unikey);
            app.login_qr_url = Some(url);
            app.login_qr_ascii = Some(ascii);
            app.login_status = "请用网易云 APP 扫码；扫码后会自动轮询状态".to_owned();
            app.logged_in = false;
        }
        ApiEvent::LoginQrStatus { code, message, logged_in } => {
            if logged_in {
                app.logged_in = true;
                app.login_status = "登录成功".to_owned();
                app.view = View::Playlists;
                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                let _ = tx.send(ApiRequest::Account).await;
            } else {
                app.login_status = format!("扫码状态 code={code} {message}");
            }
        }
        ApiEvent::SearchResult(v) => {
            app.search_results = parse_search_songs(&v);
            app.search_selected = 0;
            app.search_status = format!("结果: {} 首", app.search_results.len());
        }
        ApiEvent::SongUrlReady { id: _, url, title } => {
            app.play_status = "开始播放...".to_owned();
            let _ = tx_audio.send(AudioCommand::PlayUrl { url, title });
        }
        ApiEvent::AccountReady { uid, nickname } => {
            app.account_uid = Some(uid);
            app.account_nickname = Some(nickname);
            app.playlists_status = "正在加载用户歌单...".to_owned();
            let _ = tx.send(ApiRequest::UserPlaylists { uid }).await;
        }
        ApiEvent::UserPlaylistsReady(v) => {
            app.playlists = parse_user_playlists(&v);
            app.playlists_selected = app
                .playlists
                .iter()
                .position(|p| p.special_type == 5 || p.name.contains("我喜欢"))
                .unwrap_or(0);
            app.playlists_status = format!("歌单: {} 个，正在打开我喜欢的音乐...", app.playlists.len());
            if let Some(p) = app.playlists.get(app.playlists_selected) {
                let _ = tx.send(ApiRequest::PlaylistTracks { playlist_id: p.id }).await;
            }
        }
        ApiEvent::PlaylistTracksReady { playlist_id: _, songs } => {
            app.playlist_tracks = parse_search_songs(&songs);
            app.playlist_tracks_selected = 0;
            app.playlist_mode = PlaylistMode::Tracks;
            app.queue = app.playlist_tracks.clone();
            app.queue_pos = Some(0);
            app.playlists_status = format!("歌曲: {} 首（p 播放）", app.playlist_tracks.len());
        }
    }
}

fn handle_audio_event(app: &mut App, evt: AudioEvent) -> Option<ApiRequest> {
    match evt {
        AudioEvent::NowPlaying { play_id, title, duration_ms } => {
            app.now_playing = Some(title);
            app.paused = false;
            app.play_status = "播放中".to_owned();
            app.play_started_at = Some(Instant::now());
            app.play_total_ms = duration_ms;
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = Some(play_id);
        }
        AudioEvent::Paused(p) => {
            app.paused = p;
            app.play_status = if p { "已暂停" } else { "播放中" }.to_owned();
            if p {
                app.play_paused_at = Some(Instant::now());
            } else if let Some(t) = app.play_paused_at.take() {
                app.play_paused_accum_ms =
                    app.play_paused_accum_ms.saturating_add(t.elapsed().as_millis() as u64);
            }
        }
        AudioEvent::Stopped => {
            app.paused = false;
            app.play_status = "已停止".to_owned();
            app.play_started_at = None;
            app.play_total_ms = None;
            app.play_paused_at = None;
            app.play_paused_accum_ms = 0;
            app.play_id = None;
        }
        AudioEvent::Ended { play_id } => {
            if app.play_id != Some(play_id) {
                return None;
            }
            let Some(pos) = app.queue_pos else {
                return None;
            };
            let next = pos + 1;
            if next >= app.queue.len() {
                app.play_status = "播放结束".to_owned();
                app.queue_pos = None;
                return None;
            }
            app.queue_pos = Some(next);
            if matches!(app.view, View::Playlists) && matches!(app.playlist_mode, PlaylistMode::Tracks) {
                app.playlist_tracks_selected = next.min(app.playlist_tracks.len().saturating_sub(1));
            }
            let s = &app.queue[next];
            app.play_status = "自动下一首...".to_owned();
            let title = format!("{} - {}", s.name, s.artists);
            return Some(ApiRequest::SongUrl { id: s.id, title });
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");
        }
    }
    None
}

async fn handle_key(
    app: &mut App,
    key: KeyEvent,
    tx: &tokio_mpsc::Sender<ApiRequest>,
    tx_audio: &mpsc::Sender<AudioCommand>,
) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        } => return true,
        KeyEvent {
            code: KeyCode::Tab, ..
        } => {
            if app.logged_in {
                app.view = match app.view {
                    View::Playlists => View::Search,
                    View::Search => View::Playlists,
                    View::Login => View::Playlists,
                };
            } else {
                app.view = match app.view {
                    View::Login => View::Search,
                    View::Search => View::Login,
                    View::Playlists => View::Login,
                };
            }
        }
        _ => {}
    }

    match app.view {
        View::Login => match key.code {
            KeyCode::Char('l') => {
                if app.logged_in {
                    return false;
                }
                let _ = tx.send(ApiRequest::LoginQrKey).await;
                app.login_status = "正在生成二维码...".to_owned();
            }
            _ => {}
        },
        View::Playlists => match key.code {
            KeyCode::Char('b') => {
                app.playlist_mode = PlaylistMode::List;
                app.playlists_status = "返回歌单列表".to_owned();
            }
            KeyCode::Enter => {
                if matches!(app.playlist_mode, PlaylistMode::List) {
                    if let Some(p) = app.playlists.get(app.playlists_selected) {
                        app.playlists_status = "加载歌单歌曲中...".to_owned();
                        let _ = tx.send(ApiRequest::PlaylistTracks { playlist_id: p.id }).await;
                    }
                }
            }
            KeyCode::Char('p') => {
                if matches!(app.playlist_mode, PlaylistMode::Tracks) {
                    if let Some(s) = app.playlist_tracks.get(app.playlist_tracks_selected) {
                        app.play_status = "获取播放链接...".to_owned();
                        app.queue = app.playlist_tracks.clone();
                        app.queue_pos = Some(app.playlist_tracks_selected);
                        let title = format!("{} - {}", s.name, s.artists);
                        let _ = tx.send(ApiRequest::SongUrl { id: s.id, title }).await;
                    }
                }
            }
            KeyCode::Up => match app.playlist_mode {
                PlaylistMode::List => {
                    if app.playlists_selected > 0 {
                        app.playlists_selected -= 1;
                    }
                }
                PlaylistMode::Tracks => {
                    if app.playlist_tracks_selected > 0 {
                        app.playlist_tracks_selected -= 1;
                    }
                }
            },
            KeyCode::Down => match app.playlist_mode {
                PlaylistMode::List => {
                    if !app.playlists.is_empty() && app.playlists_selected + 1 < app.playlists.len() {
                        app.playlists_selected += 1;
                    }
                }
                PlaylistMode::Tracks => {
                    if !app.playlist_tracks.is_empty() && app.playlist_tracks_selected + 1 < app.playlist_tracks.len() {
                        app.playlist_tracks_selected += 1;
                    }
                }
            },
            KeyCode::Char(' ') => {
                let _ = tx_audio.send(AudioCommand::TogglePause);
            }
            KeyCode::Char('s') => {
                let _ = tx_audio.send(AudioCommand::Stop);
            }
            _ => {}
        },
        View::Search => match key.code {
            KeyCode::Char('p') => {
                if let Some(s) = app.search_results.get(app.search_selected) {
                    app.play_status = "获取播放链接...".to_owned();
                    app.queue.clear();
                    app.queue_pos = None;
                    let title = selected_song_title(app);
                    let _ = tx.send(ApiRequest::SongUrl { id: s.id, title }).await;
                }
            }
            KeyCode::Char(' ') => {
                let _ = tx_audio.send(AudioCommand::TogglePause);
            }
            KeyCode::Char('s') => {
                let _ = tx_audio.send(AudioCommand::Stop);
            }
            KeyCode::Enter => {
                let q = app.search_input.trim().to_owned();
                if q.is_empty() {
                    app.search_status = "请输入关键词".to_owned();
                } else {
                    let _ = tx.send(ApiRequest::Search { keywords: q }).await;
                    app.search_status = "搜索中...".to_owned();
                }
            }
            KeyCode::Backspace => {
                app.search_input.pop();
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.search_input.push(c);
                }
            }
            KeyCode::Up => {
                if app.search_selected > 0 {
                    app.search_selected -= 1;
                }
            }
            KeyCode::Down => {
                if !app.search_results.is_empty() && app.search_selected + 1 < app.search_results.len() {
                    app.search_selected += 1;
                }
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
            ["歌单", "搜索"].into_iter().map(Line::from).collect::<Vec<_>>(),
            match app.view {
                View::Playlists => 0,
                View::Search => 1,
                View::Login => 0,
            },
        )
    } else {
        (
            ["登录", "搜索"]
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
            match app.view {
                View::Login => 0,
                View::Search => 1,
                View::Playlists => 1,
            },
        )
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::default().borders(Borders::ALL).title("netease-ratui"))
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
            .map(|(i, s)| ListItem::new(Line::from(format!("{}  {} - {}", i + 1, s.name, s.artists))))
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

    let qr = app
        .login_qr_ascii
        .as_deref()
        .unwrap_or("按 l 生成二维码");
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
    let info_block = Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("信息"));
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

    let input = Paragraph::new(app.search_input.as_str())
        .block(Block::default().borders(Borders::ALL).title("关键词(回车搜索)"));
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

    draw_player_status(f, chunks[2], app, "状态", "搜索", app.search_status.as_str());
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
    let time_text = format!(
        "{} / {}{}",
        fmt_mmss(elapsed_ms),
        total_ms.map(fmt_mmss).unwrap_or_else(|| "--:--".to_owned()),
        if app.paused { " (暂停)" } else { "" }
    );

    let status = Paragraph::new(format!(
        "{}: {}\n播放: {} | Now: {}\n时间: {}\n操作: p 播放 | 空格 暂停/继续 | s 停止 | q 退出",
        context_label, context_value, app.play_status, now, time_text
    ))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(status, status_chunks[0]);

    let ratio = if let Some(total) = total_ms {
        if total == 0 { 0.0 } else { (elapsed_ms.min(total) as f64) / (total as f64) }
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("进度"))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(ratio);
    f.render_widget(gauge, status_chunks[1]);
}
