use super::{CoreState, UiAction};
use crate::core::effects::CoreEffects;
use crate::features::login as login_handlers;
use crate::messages::app::AppCommand;
use crate::netease::actor::NeteaseEvent;

pub async fn handle_ui(
    cmd: &AppCommand,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> UiAction {
    let login_cmd = match cmd {
        AppCommand::LoginGenerateQr => AppCommand::LoginGenerateQr,
        AppCommand::LoginToggleCookieInput => AppCommand::LoginToggleCookieInput,
        AppCommand::LoginCookieInputChar { c } => AppCommand::LoginCookieInputChar { c: *c },
        AppCommand::LoginCookieInputBackspace => AppCommand::LoginCookieInputBackspace,
        AppCommand::LoginCookieSubmit => AppCommand::LoginCookieSubmit,
        _ => return UiAction::NotHandled,
    };

    login_handlers::handle_login_command(
        login_cmd,
        &mut state.app,
        &mut state.req_id,
        &mut state.request_tracker,
        effects,
    )
    .await;

    UiAction::Handled
}

pub async fn handle_netease_event(
    evt: &NeteaseEvent,
    state: &mut CoreState,
    effects: &mut CoreEffects,
) -> bool {
    login_handlers::handle_login_event(
        evt,
        &mut state.app,
        &mut state.req_id,
        &mut state.request_tracker,
        &mut state.pending_playlists,
        effects,
    )
    .await
}

pub fn handle_qr_poll(state: &mut CoreState, effects: &mut CoreEffects) {
    login_handlers::handle_qr_poll(
        &state.app,
        &mut state.req_id,
        &mut state.request_tracker,
        effects,
    );
}

#[cfg(test)]
mod tests {
    use super::handle_ui;
    use crate::core::effects::CoreEffect;
    use crate::core::reducer::{CoreState, UiAction};
    use crate::messages::app::AppCommand;
    use crate::netease::actor::NeteaseCommand;

    #[tokio::test]
    async fn login_generate_qr_emits_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = CoreState::new(dir.path());
        let mut effects = crate::core::effects::CoreEffects::default();

        let outcome = handle_ui(&AppCommand::LoginGenerateQr, &mut state, &mut effects).await;

        assert!(matches!(outcome, UiAction::Handled));
        assert_eq!(state.app.login_status, "正在生成二维码...");
        assert!(effects.actions.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::SendNeteaseHi {
                    cmd: NeteaseCommand::LoginQrKey { .. },
                    ..
                }
            )
        }));
    }
}
