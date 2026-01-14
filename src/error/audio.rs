//! 音频播放相关错误

use std::path::PathBuf;

use super::{CacheError, DownloadError};

/// 音频播放错误类型
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AudioError {
    /// 打开音频文件失败
    #[error("打开音频文件失败({title}): {source}")]
    OpenFile {
        title: String,
        #[source]
        source: std::io::Error,
    },

    /// 解码音频失败
    #[error("解码音频失败({title}): {source}")]
    Decode {
        title: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// 下载错误
    #[error("下载失败: {0}")]
    Download(#[from] DownloadError),

    /// 缓存错误
    #[error("缓存操作失败: {0}")]
    Cache(#[from] CacheError),

    /// 播放器初始化失败
    #[allow(dead_code)]
    #[error("播放器初始化失败: {0}")]
    Init(String),

    /// Seek 失败
    #[allow(dead_code)]
    #[error("Seek 失败: {0}")]
    Seek(String),

    /// 音频输出流创建失败
    #[allow(dead_code)]
    #[error("创建音频输出流失败: {0}")]
    OutputStream(String),

    /// Sink 创建失败
    #[allow(dead_code)]
    #[error("创建 Sink 失败: {0}")]
    Sink(String),

    /// 文件不存在
    #[allow(dead_code)]
    #[error("音频文件不存在: {0}")]
    FileNotFound(PathBuf),
}

impl AudioError {
    /// 判断是否是可重试的错误
    #[allow(dead_code)]
    pub fn is_retryable(&self) -> bool {
        matches!(self, AudioError::Download(_) | AudioError::Seek(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_error_display() {
        let err = AudioError::Init("无法初始化音频设备".to_string());
        assert_eq!(err.to_string(), "播放器初始化失败: 无法初始化音频设备");
    }

    #[test]
    fn test_is_retryable() {
        let download_err = DownloadError::MaxRetriesExceeded { retries: 2 };
        assert!(AudioError::Download(download_err).is_retryable());

        let seek_err = AudioError::Seek("seek 失败".to_string());
        assert!(seek_err.is_retryable());

        let init_err = AudioError::Init("初始化失败".to_string());
        assert!(!init_err.is_retryable());
    }

    #[test]
    fn test_open_file_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件未找到");
        let err = AudioError::OpenFile {
            title: "测试歌曲".to_string(),
            source: io_err,
        };
        assert!(err.to_string().contains("测试歌曲"));
        assert!(err.to_string().contains("文件未找到"));
    }
}
