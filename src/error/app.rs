//! 应用通用错误

use super::{AudioError, NeteaseError};

/// 应用通用错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("JSON 序列化失败: {0}")]
    Serde(#[from] serde_json::Error),

    /// 设置错误
    #[error("设置错误: {0}")]
    Settings(#[from] SettingsError),

    /// 网易云音乐 API 错误
    #[error("网易云音乐错误: {0}")]
    Netease(#[from] NeteaseError),

    /// 音频错误
    #[error("音频错误: {0}")]
    Audio(#[from] AudioError),

    /// 数据目录错误
    #[allow(dead_code)]
    #[error("数据目录错误: {0}")]
    DataDir(String),

    /// 配置错误
    #[allow(dead_code)]
    #[error("配置错误: {0}")]
    Config(String),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

/// 设置相关错误
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// 加载设置失败
    #[allow(dead_code)]
    #[error("加载设置失败: {source}")]
    Load {
        #[source]
        source: std::io::Error,
    },

    /// 保存设置失败
    #[allow(dead_code)]
    #[error("保存设置失败: {source}")]
    Save {
        #[source]
        source: std::io::Error,
    },

    /// 解析设置失败
    #[allow(dead_code)]
    #[error("解析设置失败: {source}")]
    Parse {
        #[source]
        source: serde_json::Error,
    },

    /// 设置值无效
    #[allow(dead_code)]
    #[error("设置值无效: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件未找到");
        let err = AppError::Io(io_err);
        assert!(err.to_string().contains("IO 错误"));
    }

    #[test]
    fn test_settings_error() {
        // 使用简单的 JSON 解析错误
        let json_str = "{invalid}";
        let parse_err = serde_json::from_str::<serde_json::Value>(json_str);
        let settings_err = SettingsError::Parse {
            source: parse_err.unwrap_err(),
        };
        assert!(settings_err.to_string().contains("解析设置失败"));
    }

    #[test]
    fn test_error_chain() {
        // 测试错误链是否正确保留
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let app_err = AppError::Io(io_err);

        // 应该能获取到 source
        use std::error::Error;
        assert!(app_err.source().is_some());
    }
}
