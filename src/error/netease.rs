//! 网易云音乐 API 相关错误

/// 网易云音乐 API 错误类型
#[derive(Debug, thiserror::Error)]
pub enum NeteaseError {
    /// 网络请求错误（兼容旧的 Reqwest 变体名）
    #[error("reqwest 错误: {0}")]
    Reqwest(reqwest::Error),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(std::io::Error),

    /// 序列化错误
    #[error("serde 错误: {0}")]
    Serde(serde_json::Error),

    /// 加密错误
    #[error("crypto 错误: {0}")]
    Crypto(String),

    /// Cookie 验证失败
    #[error("Cookie 验证失败: {0}")]
    CookieValidationFailed(String),

    /// API 返回业务错误
    #[allow(dead_code)]
    #[error("API 返回错误: code={code}, msg={msg}")]
    Api { code: i32, msg: String },

    /// HTTP 头构造失败
    #[error("Header 构造失败: {0}")]
    BadHeader(String),

    /// 输入参数无效
    #[error("输入错误: {0}")]
    BadInput(&'static str),
}

// 实现 From traits 以便自动转换
impl From<reqwest::Error> for NeteaseError {
    fn from(err: reqwest::Error) -> Self {
        NeteaseError::Reqwest(err)
    }
}

impl From<std::io::Error> for NeteaseError {
    fn from(err: std::io::Error) -> Self {
        NeteaseError::Io(err)
    }
}

impl From<serde_json::Error> for NeteaseError {
    fn from(err: serde_json::Error) -> Self {
        NeteaseError::Serde(err)
    }
}

impl NeteaseError {
    /// 判断是否是网络错误
    #[allow(dead_code)]
    pub fn is_network_error(&self) -> bool {
        matches!(self, NeteaseError::Reqwest(_))
    }

    /// 判断是否是认证错误
    #[allow(dead_code)]
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            NeteaseError::CookieValidationFailed(_) | NeteaseError::Api { code: -100, .. }
        )
    }

    /// 判断是否是可重试的错误
    #[allow(dead_code)]
    pub fn is_retryable(&self) -> bool {
        matches!(self, NeteaseError::Reqwest(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netease_error_display() {
        let err = NeteaseError::BadInput("测试参数无效");
        assert_eq!(err.to_string(), "输入错误: 测试参数无效");
    }

    #[test]
    fn test_api_error() {
        let err = NeteaseError::Api {
            code: -100,
            msg: "用户未登录".to_string(),
        };
        assert!(err.to_string().contains("-100"));
        assert!(err.to_string().contains("用户未登录"));
    }

    #[test]
    fn test_is_network_error() {
        // API 错误不是网络错误
        let api_err = NeteaseError::Api {
            code: -100,
            msg: "test".to_string(),
        };
        assert!(!api_err.is_network_error());
    }

    #[test]
    fn test_is_auth_error() {
        let cookie_err = NeteaseError::CookieValidationFailed("无效".to_string());
        assert!(cookie_err.is_auth_error());

        let api_err = NeteaseError::Api {
            code: -100,
            msg: "未登录".to_string(),
        };
        assert!(api_err.is_auth_error());
    }
}
