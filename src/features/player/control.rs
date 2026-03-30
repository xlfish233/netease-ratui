use crate::core::prelude::{
    app::App,
    audio::AudioCommand,
    effects::CoreEffects,
    infra::{NextSongCacheManager, RequestKey, RequestTracker},
    messages::AppCommand,
};
use crate::features::player::playback::{play_next, play_prev, seek_absolute, seek_relative};

pub struct PlayerControlCtx<'a> {
    pub req_id: &'a mut u64,
    pub request_tracker: &'a mut RequestTracker<RequestKey>,
    pub song_request_titles: &'a mut std::collections::HashMap<i64, String>,
    pub next_song_cache: &'a mut NextSongCacheManager,
    pub effects: &'a mut CoreEffects,
}

/// 处理播放器控制相关的 AppCommand（不涉及设置持久化）
/// 返回 true 表示命令已处理，false 表示未处理
pub async fn handle_player_control_command(
    cmd: AppCommand,
    app: &mut App,
    ctx: &mut PlayerControlCtx<'_>,
) -> bool {
    tracing::debug!(
        command = ?cmd,
        paused = app.paused,
        play_song_id = ?app.play_song_id,
        play_queue_empty = app.play_queue.is_empty(),
        current_index = ?app.play_queue.current_index(),
        "🎵 [PlayerControl] 接收播放控制命令"
    );

    match cmd {
        AppCommand::PlayerTogglePause => {
            tracing::info!(
                paused = app.paused,
                current_sink_exists = app.play_song_id.is_some(),
                "🎵 [PlayerControl] 处理播放/暂停切换命令"
            );

            if app.play_song_id.is_none() {
                tracing::warn!("🎵 [PlayerControl] play_song_id 为空，无法切换播放状态");
            }

            ctx.effects.send_audio_warn(
                AudioCommand::TogglePause,
                "AudioWorker 通道已关闭：TogglePause 发送失败",
            );
        }
        AppCommand::PlayerStop => {
            ctx.effects
                .send_audio_warn(AudioCommand::Stop, "AudioWorker 通道已关闭：Stop 发送失败");
        }
        AppCommand::PlayerPrev => {
            play_prev(
                app,
                ctx.request_tracker,
                ctx.song_request_titles,
                ctx.req_id,
                ctx.next_song_cache,
                ctx.effects,
            )
            .await;
            ctx.effects.emit_state(app);
        }
        AppCommand::PlayerNext => {
            play_next(
                app,
                ctx.request_tracker,
                ctx.song_request_titles,
                ctx.req_id,
                ctx.next_song_cache,
                ctx.effects,
            )
            .await;
            ctx.effects.emit_state(app);
        }
        AppCommand::PlayerSeekBackwardMs { ms } => {
            seek_relative(app, ctx.effects, -(ms as i64));
            ctx.effects.emit_state(app);
        }
        AppCommand::PlayerSeekForwardMs { ms } => {
            seek_relative(app, ctx.effects, ms as i64);
            ctx.effects.emit_state(app);
        }
        AppCommand::PlayerSeekAbsoluteMs { ms } => {
            seek_absolute(app, ctx.effects, ms);
            ctx.effects.emit_state(app);
        }
        _ => return false,
    }
    true
}
