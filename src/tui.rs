use crate::api_worker::{ApiEvent, ApiRequest};
use crate::audio_worker::{AudioCommand, AudioEvent};
use crate::app::{parse_search_songs, App, View};
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
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
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
            handle_api_event(&mut app, evt, &tx_audio);
        }
        while let Ok(evt) = rx_audio.try_recv() {
            handle_audio_event(&mut app, evt);
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

fn handle_api_event(app: &mut App, evt: ApiEvent, tx_audio: &mpsc::Sender<AudioCommand>) {
    match evt {
        ApiEvent::Info(s) => match app.view {
            View::Login => app.login_status = s,
            View::Search => app.search_status = s,
        },
        ApiEvent::Error(e) => match app.view {
            View::Login => app.login_status = format!("错误: {e}"),
            View::Search => app.search_status = format!("错误: {e}"),
        },
        ApiEvent::ClientReady { logged_in } => {
            app.logged_in = logged_in;
            if app.logged_in {
                app.view = View::Search;
                app.search_status = "已登录（已从本地状态恢复）".to_owned();
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
                app.view = View::Search;
                app.search_status = "已登录，可直接搜索".to_owned();
            } else {
                app.login_status = format!("扫码状态 code={code} {message}");
            }
        }
        ApiEvent::SearchResult(v) => {
            app.search_results = parse_search_songs(&v);
            app.search_selected = 0;
            app.search_status = format!("结果: {} 首", app.search_results.len());
        }
        ApiEvent::SongUrlReady { id: _, url } => {
            let title = selected_song_title(app);
            app.play_status = "开始播放...".to_owned();
            let _ = tx_audio.send(AudioCommand::PlayUrl { url, title });
        }
    }
}

fn handle_audio_event(app: &mut App, evt: AudioEvent) {
    match evt {
        AudioEvent::NowPlaying { title } => {
            app.now_playing = Some(title);
            app.paused = false;
            app.play_status = "播放中".to_owned();
        }
        AudioEvent::Paused(p) => {
            app.paused = p;
            app.play_status = if p { "已暂停" } else { "播放中" }.to_owned();
        }
        AudioEvent::Stopped => {
            app.paused = false;
            app.play_status = "已停止".to_owned();
        }
        AudioEvent::Error(e) => {
            app.play_status = format!("播放错误: {e}");
        }
    }
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
                return false;
            }
            app.view = match app.view {
                View::Login => View::Search,
                View::Search => View::Login,
            };
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
        View::Search => match key.code {
            KeyCode::Char('p') => {
                if let Some(s) = app.search_results.get(app.search_selected) {
                    app.play_status = "获取播放链接...".to_owned();
                    let _ = tx.send(ApiRequest::SongUrl { id: s.id }).await;
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
            ["搜索"].into_iter().map(Line::from).collect::<Vec<_>>(),
            0,
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
        View::Search => draw_search(f, chunks[1], app),
    }
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
            Constraint::Length(5),
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

    let now = app.now_playing.as_deref().unwrap_or("-");
    let status = Paragraph::new(format!(
        "搜索: {}\n播放: {} | Now: {}\n操作: 输入/回车搜索/↑↓选择 | p 播放 | 空格 暂停/继续 | s 停止 | q 退出",
        app.search_status, app.play_status, now
    ))
    .block(Block::default().borders(Borders::ALL).title("状态"));
    f.render_widget(status, chunks[2]);
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
