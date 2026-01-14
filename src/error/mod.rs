//! 统一错误处理模块
//!
//! 提供项目中所有模块的结构化错误类型，替代 String 错误。

mod app;
mod audio;
mod cache;
mod download;
mod netease;

// 重新导出所有错误类型，便于使用
pub use app::AppError;
pub use audio::AudioError;
pub use cache::CacheError;
pub use download::DownloadError;
pub use netease::NeteaseError;

// 为方便 UI 层使用，提供 Display 的 trait impl
// 所有错误类型都通过 thiserror 自动实现了 Display 和 Error
