//! 下载相关错误

use reqwest::StatusCode;
use std::path::PathBuf;

/// 下载错误类型
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum DownloadError {
    /// HTTP 请求错误
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    /// HTTP 状态码错误
    #[error("HTTP 状态码 {status}: {url}")]
    StatusCode { status: StatusCode, url: String },

    /// 创建文件失败
    #[error("创建临时文件失败({path}): {source}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// 写入文件失败
    #[error("写入文件失败({title}): {source}")]
    Write {
        title: String,
        #[source]
        source: std::io::Error,
    },

    /// 超过最大重试次数
    #[allow(dead_code)]
    #[error("超过最大重试次数 ({retries})")]
    MaxRetriesExceeded { retries: u32 },

    /// 下载 URL 无效
    #[allow(dead_code)]
    #[error("下载 URL 无效: {0}")]
    InvalidUrl(String),
}

impl DownloadError {
    /// 判断错误是否可重试
    #[allow(dead_code)]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            DownloadError::Http(_) | DownloadError::StatusCode { .. } | DownloadError::Write { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_error_display() {
        let err = DownloadError::MaxRetriesExceeded { retries: 3 };
        assert_eq!(err.to_string(), "超过最大重试次数 (3)");
    }

    #[test]
    fn test_is_retryable() {
        // StatusCode 错误应该可重试
        assert!(
            DownloadError::StatusCode {
                status: StatusCode::REQUEST_TIMEOUT,
                url: "http://example.com".to_string()
            }
            .is_retryable()
        );

        // MaxRetriesExceeded 不可重试
        assert!(!DownloadError::MaxRetriesExceeded { retries: 3 }.is_retryable());
    }
}
