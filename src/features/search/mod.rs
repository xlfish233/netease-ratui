use crate::core::prelude::{
    app::App,
    audio::AudioCommand,
    effects::CoreEffects,
    infra::{RequestKey, RequestTracker},
    messages::AppCommand,
    netease::NeteaseCommand,
};
use crate::core::utils;

/// 处理搜索相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
#[allow(clippy::too_many_arguments)]
pub async fn handle_search_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    request_tracker: &mut RequestTracker<RequestKey>,
    song_request_titles: &mut std::collections::HashMap<i64, String>,
    effects: &mut CoreEffects,
) -> bool {
    match cmd {
        AppCommand::SearchSubmit => {
            let q = app.search_input.trim().to_owned();
            if q.is_empty() {
                app.search_status = "请输入关键词".to_owned();
                effects.emit_state(app);
                return true;
            }
            app.search_status = "搜索中...".to_owned();
            app.search_results.clear();
            app.search_selected = 0;
            effects.emit_state(app);
            let id = request_tracker.issue(RequestKey::Search, || utils::next_id(req_id));
            effects.send_netease_hi_warn(
                NeteaseCommand::CloudSearchSongs {
                    req_id: id,
                    keywords: q,
                    limit: 30,
                    offset: 0,
                },
                "NeteaseActor 通道已关闭：CloudSearchSongs 发送失败",
            );
        }
        AppCommand::SearchInputBackspace => {
            app.search_input.pop();
            effects.emit_state(app);
        }
        AppCommand::SearchInputChar { c } => {
            app.search_input.push(c);
            effects.emit_state(app);
        }
        AppCommand::SearchMoveUp => {
            if app.search_selected > 0 {
                app.search_selected -= 1;
                effects.emit_state(app);
            }
        }
        AppCommand::SearchMoveDown => {
            if !app.search_results.is_empty() && app.search_selected + 1 < app.search_results.len()
            {
                app.search_selected += 1;
                effects.emit_state(app);
            }
        }
        AppCommand::SearchPlaySelected => {
            if let Some(s) = app.search_results.get(app.search_selected) {
                app.play_status = "获取播放链接...".to_owned();
                app.play_queue.clear();
                let title = format!("{} - {}", s.name, s.artists);
                effects.emit_state(app);
                song_request_titles.clear();
                let id = request_tracker.issue(RequestKey::SongUrl, || utils::next_id(req_id));
                song_request_titles.insert(s.id, title);

                // 先停止当前播放
                effects.send_audio(AudioCommand::Stop);

                effects.send_netease_hi(NeteaseCommand::SongUrl {
                    req_id: id,
                    id: s.id,
                    br: app.play_br,
                });
            }
        }
        _ => return false,
    }
    true
}

/// 处理搜索相关的 NeteaseEvent::SearchSongs
/// req_id: 请求ID，用于匹配pending请求
/// songs: 搜索结果歌曲列表
/// 返回 true 表示事件已处理，false 表示未处理（req_id不匹配/过期）
pub async fn handle_search_songs_event(
    req_id: u64,
    songs: &[crate::domain::model::Song],
    app: &mut App,
    request_tracker: &mut RequestTracker<RequestKey>,
    effects: &mut CoreEffects,
) -> bool {
    if !request_tracker.accept(&RequestKey::Search, req_id) {
        // 过期请求，丢弃
        tracing::debug!(req_id, "搜索响应过期，丢弃");
        return false;
    }
    app.search_results = songs.to_vec();
    app.search_selected = 0;
    app.search_status = format!("结果: {} 首", app.search_results.len());
    effects.emit_state(app);
    true
}
