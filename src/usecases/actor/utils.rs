use crate::app::App;
use crate::messages::app::AppEvent;
use tokio::sync::mpsc;

/// 生成下一个请求 ID
pub(super) fn next_id(id: &mut u64) -> u64 {
    let out = *id;
    *id = id.wrapping_add(1);
    out
}

/// 推送应用状态到事件通道
pub(super) async fn push_state(tx_evt: &mpsc::Sender<AppEvent>, app: &App) {
    let _ = tx_evt.send(AppEvent::State(Box::new(app.clone()))).await;
}
