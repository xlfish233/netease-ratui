mod next_song_cache;
mod preload;
mod request_tracker;

pub use next_song_cache::NextSongCacheManager;
pub use request_tracker::{RequestKey, RequestTracker};

#[derive(Default)]
pub struct PreloadManager(pub preload::PreloadManager);

impl std::ops::Deref for PreloadManager {
    type Target = preload::PreloadManager;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PreloadManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// 重新导出 preload 模块以便访问其内部类型
pub mod preload_pub {
    pub use super::preload::*;
}
