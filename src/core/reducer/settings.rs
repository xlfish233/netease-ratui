use super::{CoreState, UiAction};
use crate::app::{UiFocus, View, tab_configs};
use crate::audio_worker::AudioCommand;
use crate::core::effects::CoreEffects;
use crate::core::utils;
use crate::features::logout;
use crate::features::playlists;
use crate::features::settings as settings_handlers;
use crate::messages::app::AppCommand;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
    data_dir: &std::path::Path,
) -> UiAction {
    match cmd {
        AppCommand::Quit => return UiAction::Quit,
        AppCommand::Bootstrap => {
            state.app.login_status = "初始化中...".to_owned();
            effects.emit_state(&state.app);
            let id = utils::next_id(&mut state.req_id);
            effects.send_netease_hi_warn(
                NeteaseCommand::Init { req_id: id },
                "NeteaseActor 通道已关闭：Init 发送失败",
            );
            return UiAction::Handled;
        }
        AppCommand::TabNext => {
            let configs = tab_configs(state.app.logged_in);
            let current_idx = configs
                .iter()
                .position(|c| c.view == state.app.view)
                .unwrap_or(0);
            let next_view = configs[(current_idx + 1) % configs.len()].view;
            state.app.view = next_view;
            state.app.ui_focus = if matches!(next_view, View::Search) {
                UiFocus::HeaderSearch
            } else {
                UiFocus::BodyCenter
            };
            effects.emit_state(&state.app);
            return UiAction::Handled;
        }
        AppCommand::TabTo { index } => {
            if let Some(&cfg) = tab_configs(state.app.logged_in).get(*index) {
                state.app.view = cfg.view;
                state.app.ui_focus = if matches!(cfg.view, View::Search) {
                    UiFocus::HeaderSearch
                } else {
                    UiFocus::BodyCenter
                };
                effects.emit_state(&state.app);
            }
            return UiAction::Handled;
        }
        AppCommand::PlayerVolumeDown | AppCommand::PlayerVolumeUp | AppCommand::PlayerCycleMode => {
            let player_cmd = match cmd {
                AppCommand::PlayerVolumeDown => AppCommand::PlayerVolumeDown,
                AppCommand::PlayerVolumeUp => AppCommand::PlayerVolumeUp,
                AppCommand::PlayerCycleMode => AppCommand::PlayerCycleMode,
                _ => unreachable!("checked by outer match"),
            };
            settings_handlers::handle_player_settings_command(
                player_cmd,
                &mut state.app,
                &mut state.settings,
                data_dir,
                effects,
                &mut state.next_song_cache,
            )
            .await;
            return UiAction::Handled;
        }
        AppCommand::SettingsMoveUp
        | AppCommand::SettingsMoveDown
        | AppCommand::SettingsDecrease
        | AppCommand::SettingsIncrease => {
            let settings_cmd = match cmd {
                AppCommand::SettingsMoveUp => AppCommand::SettingsMoveUp,
                AppCommand::SettingsMoveDown => AppCommand::SettingsMoveDown,
                AppCommand::SettingsDecrease => AppCommand::SettingsDecrease,
                AppCommand::SettingsIncrease => AppCommand::SettingsIncrease,
                _ => unreachable!("checked by outer match"),
            };
            settings_handlers::handle_settings_command(
                settings_cmd,
                &mut state.app,
                &mut state.settings,
                data_dir,
                effects,
                &mut state.next_song_cache,
            )
            .await;
            return UiAction::Handled;
        }
        AppCommand::SettingsActivate => {
            match settings_handlers::handle_settings_activate_command(&mut state.app, effects).await
            {
                Some(true) => return UiAction::Handled,
                Some(false) => {}
                None => {}
            }

            if !state.app.logged_in {
                state.app.settings_status = "未登录，无需退出".to_owned();
                effects.emit_state(&state.app);
                return UiAction::Handled;
            }

            tracing::info!("用户触发：退出登录");
            effects.send_audio_warn(AudioCommand::Stop, "AudioWorker 通道已关闭：Stop 发送失败");
            let id = utils::next_id(&mut state.req_id);
            effects.send_netease_hi_warn(
                NeteaseCommand::LogoutLocal { req_id: id },
                "NeteaseActor 通道已关闭：LogoutLocal 发送失败",
            );

            state.request_tracker.reset_all();
            state.playlist_tracks_loader = None;
            state.song_request_titles.clear();

            state.preload_mgr.reset(&mut state.app);
            state.next_song_cache.reset();
            logout::reset_app_after_logout(&mut state.app);
            state.app.login_status = "已退出登录（已清理本地cookie），按 l 重新登录".to_owned();
            effects.emit_state(&state.app);
            return UiAction::Handled;
        }
        _ => {}
    }

    UiAction::NotHandled
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    match evt {
        NeteaseEvent::Error { req_id, message } => {
            if state.next_song_cache.on_error(*req_id) {
                tracing::warn!(req_id, "预缓存失败: {}", message);
                return true;
            }

            if state.preload_mgr.on_error(&mut state.app, *req_id, message) {
                playlists::refresh_playlist_list_status(&mut state.app);
                effects.emit_state(&state.app);
                return true;
            }

            match state.app.view {
                View::Login => state.app.login_status = format!("错误: {message}"),
                View::Playlists => state.app.playlists_status = format!("错误: {message}"),
                View::Search => state.app.search_status = format!("错误: {message}"),
                View::Lyrics => state.app.lyrics_status = format!("错误: {message}"),
                View::Settings => state.app.settings_status = format!("错误: {message}"),
            }
            effects.emit_state(&state.app);
            true
        }
        NeteaseEvent::LoggedOut { req_id } => {
            tracing::debug!(req_id, "NeteaseActor: LoggedOut");
            true
        }
        NeteaseEvent::AnonymousReady { req_id } => {
            tracing::debug!(req_id, "NeteaseActor: AnonymousReady");
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::handle_ui;
    use crate::app::View;
    use crate::audio_worker::AudioCommand;
    use crate::core::effects::CoreEffect;
    use crate::core::reducer::{CoreState, UiAction};
    use crate::messages::app::AppCommand;

    #[tokio::test]
    async fn settings_activate_clear_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        state.app.view = View::Settings;
        state.app.settings_selected = 5;

        let outcome = handle_ui(
            &AppCommand::SettingsActivate,
            &mut state,
            &mut effects,
            dir.path(),
        )
        .await;

        assert!(matches!(outcome, UiAction::Handled));
        assert_eq!(state.app.settings_status, "正在清除音频缓存...");
        assert!(effects.actions.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::SendAudio {
                    cmd: AudioCommand::ClearCache,
                    ..
                }
            )
        }));
    }
}
