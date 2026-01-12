// TUI 子模块
mod event_loop;
mod guard;
mod keyboard;
mod login_view;
mod lyrics_view;
mod mouse;
mod player_status;
mod playlists_view;
mod search_view;
mod settings_view;
mod utils;
mod views;
mod widgets;

use crate::app::App;
use crate::messages::app::{AppCommand, AppEvent};
use std::io;
use tokio::sync::mpsc;

/// 主 TUI 入口点 - 从 main.rs 调用
pub async fn run_tui(
    app: App,
    tx: mpsc::Sender<AppCommand>,
    rx: mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    event_loop::run_tui_internal(app, tx, rx).await
}
