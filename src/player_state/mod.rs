mod store;

pub use store::{apply_snapshot_to_app, load_player_state_async, save_player_state_async};

// 重新导出 PlayerStateError 从统一错误模块
pub use crate::error::PlayerStateError;

// 同步版本主要用于测试/非 async 场景；在主循环已切换为 async 版本。
#[allow(unused_imports)]
pub use store::{load_player_state, save_player_state};
