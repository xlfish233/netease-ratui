// TUI 子模块
mod event_loop;
mod guard;
mod header;
mod keyboard;
mod layout;
mod login_view;
mod lyrics_view;
mod mouse;
mod overlays;
mod player_status;
mod panels;
mod playlists_view;
mod search_view;
mod settings_view;
mod styles;
mod utils;
mod views;
mod widgets;

use crate::app::AppSnapshot;
use crate::messages::app::{AppCommand, AppEvent};
use std::io;
use tokio::sync::mpsc;

/// 主 TUI 入口点 - 从 main.rs 调用
pub async fn run_tui(
    app: AppSnapshot,
    tx: mpsc::Sender<AppCommand>,
    rx: mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    event_loop::run_tui_internal(app, tx, rx).await
}
