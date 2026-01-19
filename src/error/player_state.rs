//! 播放器状态持久化相关错误

use std::path::PathBuf;

/// 播放器状态持久化错误类型
#[derive(Debug, thiserror::Error)]
pub enum PlayerStateError {
    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("JSON 序列化失败: {0}")]
    Serde(#[from] serde_json::Error),

    /// 版本不兼容
    #[error("版本不兼容: 预期 {expected}, 找到 {found}")]
    IncompatibleVersion { expected: u8, found: u8 },

    /// 文件不存在
    #[allow(dead_code)]
    #[error("状态文件不存在: {0}")]
    FileNotFound(PathBuf),
}

impl PlayerStateError {
    /// 判断是否是可重试的错误
    #[allow(dead_code)]
    pub fn is_retryable(&self) -> bool {
        matches!(self, PlayerStateError::Io(_) | PlayerStateError::Serde(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_state_error_display() {
        let err = PlayerStateError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(err.to_string().contains("IO 错误"));

        let err = PlayerStateError::IncompatibleVersion {
            expected: 3,
            found: 2,
        };
        assert!(err.to_string().contains("版本不兼容"));
    }

    #[test]
    fn test_is_retryable() {
        let io_err = PlayerStateError::Io(std::io::Error::other("test"));
        assert!(io_err.is_retryable());

        let version_err = PlayerStateError::IncompatibleVersion {
            expected: 3,
            found: 2,
        };
        assert!(!version_err.is_retryable());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let state_err = PlayerStateError::from(io_err);
        assert!(matches!(state_err, PlayerStateError::Io(_)));
        assert!(state_err.to_string().contains("access denied"));
    }

    #[test]
    fn test_from_serde_error() {
        let json_str = "{invalid}";
        let serde_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let state_err = PlayerStateError::from(serde_err);
        assert!(matches!(state_err, PlayerStateError::Serde(_)));
    }

    #[test]
    fn test_error_chain() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let state_err = PlayerStateError::Io(io_err);

        // 应该能获取到 source
        use std::error::Error;
        assert!(state_err.source().is_some());
    }
}
