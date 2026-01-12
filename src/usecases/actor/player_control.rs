use crate::app::App;
use crate::audio_worker::AudioCommand;
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::actor::NeteaseCommand;
use crate::usecases::actor::playback;

use super::utils;
use tokio::sync::mpsc;

pub(super) struct PlayerControlCtx<'a> {
    pub req_id: &'a mut u64,
    pub pending_song_url: &'a mut Option<(u64, String)>,
    pub tx_audio: &'a std::sync::mpsc::Sender<AudioCommand>,
    pub tx_netease_hi: &'a mpsc::Sender<NeteaseCommand>,
    pub tx_netease_lo: &'a mpsc::Sender<NeteaseCommand>,
    pub tx_evt: &'a mpsc::Sender<AppEvent>,
    pub next_song_cache: &'a mut super::next_song_cache::NextSongCacheManager,
}

/// 处理播放器控制相关的 AppCommand（不涉及设置持久化）
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_player_control_command(
    cmd: AppCommand,
    app: &mut App,
    ctx: &mut PlayerControlCtx<'_>,
) -> bool {
    match cmd {
        AppCommand::PlayerTogglePause => {
            if ctx.tx_audio.send(AudioCommand::TogglePause).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：TogglePause 发送失败");
            }
        }
        AppCommand::PlayerStop => {
            if ctx.tx_audio.send(AudioCommand::Stop).is_err() {
                tracing::warn!("AudioWorker 通道已关闭：Stop 发送失败");
            }
        }
        AppCommand::PlayerPrev => {
            playback::play_prev(
                app,
                ctx.tx_netease_hi,
                ctx.pending_song_url,
                ctx.req_id,
                ctx.next_song_cache,
                ctx.tx_netease_lo,
            )
            .await;
            utils::push_state(ctx.tx_evt, app).await;
        }
        AppCommand::PlayerNext => {
            playback::play_next(
                app,
                ctx.tx_netease_hi,
                ctx.pending_song_url,
                ctx.req_id,
                ctx.next_song_cache,
                ctx.tx_netease_lo,
            )
            .await;
            utils::push_state(ctx.tx_evt, app).await;
        }
        AppCommand::PlayerSeekBackwardMs { ms } => {
            playback::seek_relative(app, ctx.tx_audio, -(ms as i64));
            utils::push_state(ctx.tx_evt, app).await;
        }
        AppCommand::PlayerSeekForwardMs { ms } => {
            playback::seek_relative(app, ctx.tx_audio, ms as i64);
            utils::push_state(ctx.tx_evt, app).await;
        }
        _ => return false,
    }
    true
}
