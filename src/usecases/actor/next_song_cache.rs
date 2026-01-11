//! 下一首歌预缓存管理器

use crate::app::{App, PlayMode};
use crate::audio_worker::AudioCommand;
use crate::domain::model::SongUrl;
use crate::netease::actor::NeteaseCommand;
use tokio::sync::mpsc;

#[derive(Default)]
pub struct NextSongCacheManager {
    generation: u64,
    pending_req_id: Option<u64>,
    cached_song_id: Option<i64>,
}

impl NextSongCacheManager {
    /// 失效当前预缓存状态（队列改变、模式切换时调用）
    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending_req_id = None;
        self.cached_song_id = None;
    }

    /// 检查 req_id 是否属于预缓存请求
    pub fn owns_req(&self, req_id: u64) -> bool {
        self.pending_req_id == Some(req_id)
    }

    /// 触发预缓存下一首
    pub async fn prefetch_next(
        &mut self,
        app: &App,
        tx_netease_lo: &mpsc::Sender<NeteaseCommand>,
        req_id: &mut u64,
    ) {
        // 边界检查
        if app.queue.is_empty() || app.queue_pos.is_none() {
            return;
        }

        // Shuffle 模式不预缓存（不可预测）
        if matches!(app.play_mode, PlayMode::Shuffle) {
            return;
        }

        // SingleLoop 模式不预缓存（下一首是当前，已缓存）
        if matches!(app.play_mode, PlayMode::SingleLoop) {
            return;
        }

        // 计算下一首索引
        let Some(next_idx) = super::playback::calculate_next_index(app) else {
            return; // Sequential 到末尾，无下一首
        };

        let Some(next_song) = app.queue.get(next_idx) else {
            return;
        };

        // 检查是否已经缓存过这首
        if self.cached_song_id == Some(next_song.id) {
            return;
        }

        // 发起预缓存请求
        let id = crate::usecases::actor::utils::next_id(req_id);
        self.pending_req_id = Some(id);

        let _ = tx_netease_lo
            .send(NeteaseCommand::SongUrl {
                req_id: id,
                id: next_song.id,
                br: app.play_br,
            })
            .await;
    }

    /// 处理 SongUrl 响应，发送 PrefetchAudio 命令
    pub fn on_song_url(
        &mut self,
        req_id: u64,
        song_url: &SongUrl,
        tx_audio: &std::sync::mpsc::Sender<AudioCommand>,
        app: &App,
    ) -> bool {
        if self.pending_req_id != Some(req_id) {
            return false;
        }

        self.pending_req_id = None;

        // 发送预缓存命令到 Audio Worker
        let title = format!("预缓存: {}", song_url.id);
        let _ = tx_audio.send(AudioCommand::PrefetchAudio {
            id: song_url.id,
            br: app.play_br,
            url: song_url.url.clone(),
            title,
        });

        self.cached_song_id = Some(song_url.id);
        true
    }

    /// 处理预缓存请求错误
    pub fn on_error(&mut self, req_id: u64) -> bool {
        if self.owns_req(req_id) {
            self.pending_req_id = None;
            true
        } else {
            false
        }
    }
}
