use crate::core::prelude::{
    app::App,
    effects::CoreEffects,
    infra::{RequestKey, RequestTracker},
    messages::AppCommand,
    netease::{NeteaseCommand, NeteaseEvent},
};
use crate::core::utils;

/// 处理登录相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示需要 continue
pub async fn handle_login_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    request_tracker: &mut RequestTracker<RequestKey>,
    effects: &mut CoreEffects,
) -> bool {
    match cmd {
        AppCommand::LoginGenerateQr => {
            if app.logged_in {
                return true; // 已登录，需要 continue
            }
            app.login_status = "正在生成二维码...".to_owned();
            effects.emit_state(app);
            let id = request_tracker.issue(RequestKey::LoginQrKey, || utils::next_id(req_id));
            effects.send_netease_hi_warn(
                NeteaseCommand::LoginQrKey { req_id: id },
                "NeteaseActor 通道已关闭：LoginQrKey 发送失败",
            );
        }
        AppCommand::LoginToggleCookieInput => {
            app.login_cookie_input_visible = !app.login_cookie_input_visible;
            app.login_cookie_input.clear();
            app.login_status = if app.login_cookie_input_visible {
                "Cookie 输入模式：输入 MUSIC_U 值".to_owned()
            } else {
                "按 l 生成二维码；按 c 切换到 Cookie 登录".to_owned()
            };
            effects.emit_state(app);
        }
        AppCommand::LoginCookieInputChar { c } => {
            app.login_cookie_input.push(c);
            effects.emit_state(app);
        }
        AppCommand::LoginCookieInputBackspace => {
            app.login_cookie_input.pop();
            effects.emit_state(app);
        }
        AppCommand::LoginCookieSubmit => {
            let music_u = app.login_cookie_input.trim().to_owned();
            if music_u.is_empty() {
                app.login_status = "请输入 MUSIC_U 值".to_owned();
                effects.emit_state(app);
                return true; // 空输入，需要 continue
            }
            app.login_status = "正在验证 Cookie...".to_owned();
            effects.emit_state(app);
            let id = request_tracker.issue(RequestKey::LoginSetCookie, || utils::next_id(req_id));
            effects.send_netease_hi_warn(
                NeteaseCommand::LoginSetCookie {
                    req_id: id,
                    music_u,
                },
                "NeteaseActor 通道已关闭：LoginSetCookie 发送失败",
            );
        }
        _ => return false,
    }
    false
}

/// 处理登录相关的 NeteaseEvent
/// 返回 true 表示事件已处理，false 表示需要跳过
pub async fn handle_login_event(
    evt: &NeteaseEvent,
    app: &mut App,
    req_id: &mut u64,
    request_tracker: &mut RequestTracker<RequestKey>,
    pending_playlists: &mut Option<u64>,
    effects: &mut CoreEffects,
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
                effects.emit_state(app);
                let id = request_tracker.issue(RequestKey::Account, || utils::next_id(req_id));
                effects.send_netease_hi_warn(
                    NeteaseCommand::UserAccount { req_id: id },
                    "NeteaseActor 通道已关闭：UserAccount 发送失败",
                );
            } else {
                app.login_status = "按 l 生成二维码；q 退出；Tab 切换页面".to_owned();
                effects.emit_state(app);
            }
            true
        }
        NeteaseEvent::LoginQrKey { req_id: id, unikey } => {
            if !request_tracker.accept(&RequestKey::LoginQrKey, *id) {
                tracing::debug!(req_id = id, "LoginQrKey 响应过期，丢弃");
                return false;
            }
            app.login_unikey = Some(unikey.clone());
            app.login_qr_url = Some(format!("https://music.163.com/login?codekey={unikey}"));
            app.login_qr_ascii = Some(render_qr_ascii(app.login_qr_url.as_deref().unwrap_or("")));
            app.login_status = "请用网易云 APP 扫码；扫码后会自动轮询状态".to_owned();
            app.logged_in = false;
            effects.emit_state(app);
            true
        }
        NeteaseEvent::LoginQrStatus { req_id: id, status } => {
            if !request_tracker.accept(&RequestKey::LoginQrPoll, *id) {
                tracing::debug!(req_id = id, "LoginQrStatus 响应过期，丢弃");
                return false;
            }
            if status.logged_in {
                app.logged_in = true;
                app.login_status = "登录成功".to_owned();
                app.view = crate::app::View::Playlists;
                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                effects.emit_state(app);
                effects.toast("扫码登录成功");
                let id = request_tracker.issue(RequestKey::Account, || utils::next_id(req_id));
                effects.send_netease_hi_warn(
                    NeteaseCommand::UserAccount { req_id: id },
                    "NeteaseActor 通道已关闭：UserAccount 发送失败",
                );
            } else {
                app.login_status = format!("扫码状态 code={} {}", status.code, status.message);
                effects.emit_state(app);
            }
            true
        }
        NeteaseEvent::LoginCookieSet {
            req_id: id,
            success,
            message,
        } => {
            if !request_tracker.accept(&RequestKey::LoginSetCookie, *id) {
                tracing::debug!(req_id = id, "LoginCookieSet 响应过期，丢弃");
                return false;
            }
            if *success {
                app.login_cookie_input.clear();
                app.login_cookie_input_visible = false;
                app.logged_in = true;
                app.login_status = message.clone();
                app.view = crate::app::View::Playlists;
                app.playlists_status = "登录成功，正在加载账号信息...".to_owned();
                effects.emit_state(app);
                effects.toast("Cookie 登录成功");
                let id = request_tracker.issue(RequestKey::Account, || utils::next_id(req_id));
                effects.send_netease_hi_warn(
                    NeteaseCommand::UserAccount { req_id: id },
                    "NeteaseActor 通道已关闭：UserAccount 发送失败",
                );
            } else {
                app.login_status = format!("验证失败: {message}");
                effects.emit_state(app);
                effects.error(format!("Cookie 验证失败: {message}"));
            }
            true
        }
        NeteaseEvent::Account {
            req_id: id,
            account,
        } => {
            if !request_tracker.accept(&RequestKey::Account, *id) {
                tracing::debug!(req_id = id, "Account 响应过期，丢弃");
                return false;
            }
            app.account_uid = Some(account.uid);
            app.account_nickname = Some(account.nickname.clone());
            app.playlists_status = "正在加载用户歌单...".to_owned();
            effects.emit_state(app);
            // 发送 UserPlaylists 请求
            let id = utils::next_id(req_id);
            *pending_playlists = Some(id);
            effects.send_netease_hi_warn(
                NeteaseCommand::UserPlaylists {
                    req_id: id,
                    uid: app.account_uid.unwrap_or_default(),
                },
                "NeteaseActor 通道已关闭：UserPlaylists 发送失败",
            );
            true
        }
        _ => false,
    }
}

/// 处理 QrPoll 定时器事件
pub fn handle_qr_poll(
    app: &App,
    req_id: &mut u64,
    request_tracker: &mut RequestTracker<RequestKey>,
    effects: &mut CoreEffects,
) {
    if let Some(key) = app.login_unikey.as_ref().filter(|_| !app.logged_in) {
        let id = request_tracker.issue(RequestKey::LoginQrPoll, || utils::next_id(req_id));
        effects.send_netease_hi_warn(
            NeteaseCommand::LoginQrCheck {
                req_id: id,
                key: key.clone(),
            },
            "NeteaseActor 通道已关闭：LoginQrCheck 发送失败",
        );
    }
}

/// 渲染二维码为 ASCII 字符串
pub fn render_qr_ascii(url: &str) -> String {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return "二维码生成失败".to_owned();
    };
    code.render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build()
}
