use crate::app::App;
use crate::audio_worker::AudioCommand;
use crate::messages::app::{AppCommand, AppEvent};
use crate::netease::actor::NeteaseCommand;

use super::utils;
use tokio::sync::mpsc;

/// 处理搜索相关的 AppCommand
/// 返回 true 表示命令已处理，false 表示未处理
pub(super) async fn handle_search_command(
    cmd: AppCommand,
    app: &mut App,
    req_id: &mut u64,
    pending_search: &mut Option<u64>,
    pending_song_url: &mut Option<(u64, String)>,
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    match cmd {
        AppCommand::SearchSubmit => {
            let q = app.search_input.trim().to_owned();
            if q.is_empty() {
                app.search_status = "请输入关键词".to_owned();
                utils::push_state(tx_evt, app).await;
                return true;
            }
            app.search_status = "搜索中...".to_owned();
            app.search_results.clear();
            app.search_selected = 0;
            utils::push_state(tx_evt, app).await;
            let id = utils::next_id(req_id);
            *pending_search = Some(id);
            if let Err(e) = tx_netease_hi
                .send(NeteaseCommand::CloudSearchSongs {
                    req_id: id,
                    keywords: q,
                    limit: 30,
                    offset: 0,
                })
                .await
            {
                tracing::warn!(err = %e, "NeteaseActor 通道已关闭：CloudSearchSongs 发送失败");
            }
        }
        AppCommand::SearchInputBackspace => {
            app.search_input.pop();
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::SearchInputChar { c } => {
            app.search_input.push(c);
            utils::push_state(tx_evt, app).await;
        }
        AppCommand::SearchMoveUp => {
            if app.search_selected > 0 {
                app.search_selected -= 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::SearchMoveDown => {
            if !app.search_results.is_empty() && app.search_selected + 1 < app.search_results.len()
            {
                app.search_selected += 1;
                utils::push_state(tx_evt, app).await;
            }
        }
        AppCommand::SearchPlaySelected => {
            if let Some(s) = app.search_results.get(app.search_selected) {
                app.play_status = "获取播放链接...".to_owned();
                app.queue.clear();
                app.queue_pos = None;
                let title = format!("{} - {}", s.name, s.artists);
                utils::push_state(tx_evt, app).await;
                let id = utils::next_id(req_id);
                *pending_song_url = Some((id, title.clone()));

                // 先停止当前播放
                let _ = tx_audio.send(AudioCommand::Stop);

                let _ = tx_netease_hi
                    .send(NeteaseCommand::SongUrl {
                        req_id: id,
                        id: s.id,
                        br: app.play_br,
                    })
                    .await;
            }
        }
        _ => return false,
    }
    true
}

/// 处理搜索相关的 NeteaseEvent::SearchSongs
/// req_id: 请求ID，用于匹配pending请求
/// songs: 搜索结果歌曲列表
/// 返回 true 表示事件已处理，false 表示未处理（req_id不匹配）
pub(super) async fn handle_search_songs_event(
    req_id: u64,
    songs: &[crate::domain::model::Song],
    app: &mut App,
    pending_search: &mut Option<u64>,
    tx_evt: &mpsc::Sender<AppEvent>,
) -> bool {
    if *pending_search != Some(req_id) {
        return false;
    }
    *pending_search = None;
    app.search_results = songs.to_vec();
    app.search_selected = 0;
    app.search_status = format!("结果: {} 首", app.search_results.len());
    utils::push_state(tx_evt, app).await;
    true
}
