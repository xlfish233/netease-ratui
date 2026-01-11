use crate::app::App;
use crate::audio_worker::AudioCommand;
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::actor::NeteaseCommand;
use crate::usecases::actor::playback;

use super::utils;
use tokio::sync::mpsc;

/// 处理播放器控制相关的 AppCommand（不涉及设置持久化）
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_player_control_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    pending_song_url: &mut Option<(u64, String)>,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    match cmd {
        AppCommand::PlayerTogglePause => {
            if tx_audio.send(AudioCommand::TogglePause).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：TogglePause 发送失败");
            }
        }
        AppCommand::PlayerStop => {
            if tx_audio.send(AudioCommand::Stop).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：Stop 发送失败");
            }
        }
        AppCommand::PlayerPrev => {
            playback::play_prev(app, tx_netease_hi, pending_song_url, req_id).await;
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::PlayerNext => {
            playback::play_next(app, tx_netease_hi, pending_song_url, req_id).await;
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::PlayerSeekBackwardMs { ms } => {
            playback::seek_relative(app, tx_audio, -(ms as i64));
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::PlayerSeekForwardMs { ms } => {
            playback::seek_relative(app, tx_audio, ms as i64);
            utils::push_state(tx_evt, app).await;
        }
        _ => return false,
    }
    true
}
