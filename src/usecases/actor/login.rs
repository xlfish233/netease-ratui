use crate::app::App;
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};

use super::utils;
use tokio::sync::mpsc;

/// 处理登录相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示需要 continue
pub(super) async fn handle_login_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    pending_login_qr_key: &mut Option<u64>,
    pending_login_set_cookie: &mut Option<u64>,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    match cmd {
        AppCommand::LoginGenerateQr => {
            if app.logged_in {
                return true; // 已登录，需要 continue
            }
            app.login_status = "正在生成二维码...".to_owned();
            utils::push_state(tx_evt, app).await;
            let id = utils::next_id(req_id);
            *pending_login_qr_key = Some(id);
            if let Err(e) = tx_netease_hi
                .send(NeteaseCommand::LoginQrKey { req_id: id })
                .await
            {
                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：LoginQrKey 发送失败");
            }
        }
        AppCommand::LoginToggleCookieInput => {
            app.login_cookie_input_visible = !app.login_cookie_input_visible;
            app.login_cookie_input.clear();
            app.login_status = if app.login_cookie_input_visible {
                "Cookie 输入模式：输入 MUSIC_U 值".to_owned()
            } else {
                "按 l 生成二维码；按 c 切换到 Cookie 登录".to_owned()
            };
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::LoginCookieInputChar { c } => {
            app.login_cookie_input.push(c);
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::LoginCookieInputBackspace => {
            app.login_cookie_input.pop();
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::LoginCookieSubmit => {
            let music_u = app.login_cookie_input.trim().to_owned();
            if music_u.is_empty() {
                app.login_status = "请输入 MUSIC_U 值".to_owned();
                utils::push_state(tx_evt, app).await;
                return true; // 空输入，需要 continue
            }
            app.login_status = "正在验证 Cookie...".to_owned();
            utils::push_state(tx_evt, app).await;
            let id = utils::next_id(req_id);
            *pending_login_set_cookie = Some(id);
            if let Err(e) = tx_netease_hi
                .send(NeteaseCommand::LoginSetCookie {
                    req_id: id,
                    music_u,
                })
                .await
            {
                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：LoginSetCookie 发送失败");
            }
        }
        _ => return false,
    }
    false
}

/// 处理登录相关的 NeteaseEvent
/// 返回 true 表示事件已处理，false 表示需要跳过
#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_login_event(
    evt: &NeteaseEvent,
    app: &mut App,
    req_id: &mut u64,
    pending_login_qr_key: &mut Option<u64>,
    pending_login_poll: &mut Option<u64>,
    pending_login_set_cookie: &mut Option<u64>,
    pending_account: &mut Option<u64>,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    match evt {
        NeteaseEvent::ClientReady {
            req_id: evt_req_id,
            logged_in,
        } => {
            tracing::debug!(req_id = evt_req_id, logged_in, "NeteaseActor: ClientReady");
            app.logged_in = *logged_in;
            if app.logged_in {
                app.view = crate::app::View::Playlists;
                app.playlists_status = "已登录（已从本地状态恢复），正在加载账号信息...".to_owned();
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_account = Some(id);
                if let Err(e) = tx_netease_hi
                    .send(NeteaseCommand::UserAccount { req_id: id })
                    .await
                {
                    tracing::warn!(err = %e, "NeteaseActor 通道已关闭：UserAccount 发送失败");
                }
            } else {
                app.login_status = "按 l 生成二维码；q 退出；Tab 切换页面".to_owned();
                utils::push_state(tx_evt, app).await;
            }
            true
        }
        NeteaseEvent::LoginQrKey { req_id: id, unikey } => {
            if *pending_login_qr_key != Some(*id) {
                return false;
            }
            *pending_login_qr_key = None;
            app.login_unikey = Some(unikey.clone());
            app.login_qr_url = Some(format!("https://music.163.com/login?codekey={unikey}"));
            app.login_qr_ascii = Some(render_qr_ascii(app.login_qr_url.as_deref().unwrap_or("")));
            app.login_status = "请用网易云 APP 扫码；扫码后会自动轮询状态".to_owned();
            app.logged_in = false;
            utils::push_state(tx_evt, app).await;
            true
        }
        NeteaseEvent::LoginQrStatus { req_id: id, status } => {
            if *pending_login_poll != Some(*id) {
                return false;
            }
            if status.logged_in {
                app.logged_in = true;
                app.login_status = "登录成功".to_owned();
                app.view = crate::app::View::Playlists;
                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_account = Some(id);
                if let Err(e) = tx_netease_hi
                    .send(NeteaseCommand::UserAccount { req_id: id })
                    .await
                {
                    tracing::warn!(err = %e, "NeteaseActor 通道已关闭：UserAccount 发送失败");
                }
            } else {
                app.login_status = format!("扫码状态 code={} {}", status.code, status.message);
                utils::push_state(tx_evt, app).await;
            }
            true
        }
        NeteaseEvent::LoginCookieSet {
            req_id: id,
            success,
            message,
        } => {
            if *pending_login_set_cookie != Some(*id) {
                return false;
            }
            *pending_login_set_cookie = None;
            if *success {
                app.login_cookie_input.clear();
                app.login_cookie_input_visible = false;
                app.logged_in = true;
                app.login_status = message.clone();
                app.view = crate::app::View::Playlists;
                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_account = Some(id);
                if let Err(e) = tx_netease_hi
                    .send(NeteaseCommand::UserAccount { req_id: id })
                    .await
                {
                    tracing::warn!(err = %e, "NeteaseActor 通道已关闭：UserAccount 发送失败");
                }
            } else {
                app.login_status = format!("验证失败: {message}");
                utils::push_state(tx_evt, app).await;
            }
            true
        }
        _ => false,
    }
}

/// 渲染二维码为 ASCII 字符串
pub(super) fn render_qr_ascii(url: &str) -> String {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return "二维码生成失败".to_owned();
    };
    code.render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build()
}
