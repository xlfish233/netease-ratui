//! 跨 Actor 边界的统一错误类型
//!
//! MessageError 用于在消息传递层（Actor 之间）传递结构化错误信息，
//! 避免使用 String 导致的错误上下文丢失。

/// 轻量级应用错误变体（用于跨边界传递）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppErrorVariant {
    Io(String),
    Serde(String),
    Settings(String),
    Netease(NeteaseErrorVariant),
    Audio(AudioErrorVariant),
    DataDir(String),
    Config(String),
    Other(String),
}

/// 轻量级网易云音乐错误变体
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeteaseErrorVariant {
    Reqwest(String),
    Io(String),
    Serde(String),
    Crypto(String),
    CookieValidationFailed(String),
    Api { code: i32, msg: String },
    BadHeader(String),
    BadInput(&'static str),
}

/// 轻量级音频错误变体
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioErrorVariant {
    OpenFile { title: String, source: String },
    Decode { title: String, source: String },
    Download(String),
    Cache(String),
    Init(String),
    Seek(String),
    OutputStream(String),
    Sink(String),
    FileNotFound(String),
}

impl From<crate::error::NeteaseError> for NeteaseErrorVariant {
    fn from(err: crate::error::NeteaseError) -> Self {
        match err {
            crate::error::NeteaseError::Reqwest(e) => NeteaseErrorVariant::Reqwest(e.to_string()),
            crate::error::NeteaseError::Io(e) => NeteaseErrorVariant::Io(e.to_string()),
            crate::error::NeteaseError::Serde(e) => NeteaseErrorVariant::Serde(e.to_string()),
            crate::error::NeteaseError::Crypto(s) => NeteaseErrorVariant::Crypto(s),
            crate::error::NeteaseError::CookieValidationFailed(s) => {
                NeteaseErrorVariant::CookieValidationFailed(s)
            }
            crate::error::NeteaseError::Api { code, msg } => NeteaseErrorVariant::Api { code, msg },
            crate::error::NeteaseError::BadHeader(s) => NeteaseErrorVariant::BadHeader(s),
            crate::error::NeteaseError::BadInput(s) => NeteaseErrorVariant::BadInput(s),
        }
    }
}

impl From<crate::error::AudioError> for AudioErrorVariant {
    fn from(err: crate::error::AudioError) -> Self {
        match &err {
            crate::error::AudioError::OpenFile { title, source } => AudioErrorVariant::OpenFile {
                title: title.clone(),
                source: source.to_string(),
            },
            crate::error::AudioError::Decode { title, source } => AudioErrorVariant::Decode {
                title: title.clone(),
                source: source.to_string(),
            },
            crate::error::AudioError::Download(e) => AudioErrorVariant::Download(e.to_string()),
            crate::error::AudioError::Cache(e) => AudioErrorVariant::Cache(e.to_string()),
            crate::error::AudioError::Init(s) => AudioErrorVariant::Init(s.clone()),
            crate::error::AudioError::Seek(s) => AudioErrorVariant::Seek(s.clone()),
            crate::error::AudioError::OutputStream(s) => AudioErrorVariant::OutputStream(s.clone()),
            crate::error::AudioError::Sink(s) => AudioErrorVariant::Sink(s.clone()),
            crate::error::AudioError::FileNotFound(p) => {
                AudioErrorVariant::FileNotFound(p.display().to_string())
            }
        }
    }
}

impl From<crate::error::AppError> for AppErrorVariant {
    fn from(err: crate::error::AppError) -> Self {
        match err {
            crate::error::AppError::Io(e) => AppErrorVariant::Io(e.to_string()),
            crate::error::AppError::Serde(e) => AppErrorVariant::Serde(e.to_string()),
            crate::error::AppError::Settings(e) => AppErrorVariant::Settings(e.to_string()),
            crate::error::AppError::Netease(e) => AppErrorVariant::Netease(e.into()),
            crate::error::AppError::Audio(e) => AppErrorVariant::Audio(e.into()),
            crate::error::AppError::DataDir(s) => AppErrorVariant::DataDir(s),
            crate::error::AppError::Config(s) => AppErrorVariant::Config(s),
            crate::error::AppError::Other(s) => AppErrorVariant::Other(s),
        }
    }
}

/// 错误上下文标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorContext {
    Login,
    Search,
    Playlist,
    Playback,
    Lyric,
    Settings,
    Cache,
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorContext::Login => write!(f, "登录"),
            ErrorContext::Search => write!(f, "搜索"),
            ErrorContext::Playlist => write!(f, "歌单"),
            ErrorContext::Playback => write!(f, "播放"),
            ErrorContext::Lyric => write!(f, "歌词"),
            ErrorContext::Settings => write!(f, "设置"),
            ErrorContext::Cache => write!(f, "缓存"),
        }
    }
}

/// 跨 Actor 边界的统一错误类型
///
/// 设计目标：
/// 1. 可 Clone - 允许事件包含错误并多次传递
/// 2. 轻量级 - 不包含完整的错误链，但保留关键信息
/// 3. 结构化 - 保留错误类型和上下文，便于分类处理
#[derive(Debug, Clone, thiserror::Error)]
pub enum MessageError {
    /// 应用层错误
    #[error("应用错误: {0}")]
    App(AppErrorVariant),

    /// 网易云音乐错误
    #[error("网易云音乐错误: {0}")]
    Netease(NeteaseErrorVariant),

    /// 音频错误
    #[error("音频错误: {0}")]
    Audio(AudioErrorVariant),

    /// 带上下文的错误
    #[error("{context}: {message}")]
    #[allow(dead_code)]
    WithContext {
        context: ErrorContext,
        message: String,
    },

    /// 通用错误（保持向后兼容）
    #[error("{0}")]
    Other(String),
}

impl MessageError {
    /// 判断是否是可重试的错误
    pub fn is_retryable(&self) -> bool {
        match self {
            MessageError::Netease(e) => matches!(
                e,
                NeteaseErrorVariant::Reqwest(_) | NeteaseErrorVariant::Io(_)
            ),
            MessageError::Audio(e) => matches!(
                e,
                AudioErrorVariant::Download(_) | AudioErrorVariant::Seek(_)
            ),
            MessageError::WithContext { .. } => true,
            _ => false,
        }
    }

    /// 转换为显示字符串
    #[allow(dead_code)]
    pub fn to_display_string(&self) -> String {
        self.to_string()
    }

    /// 创建带上下文的错误
    #[allow(dead_code)]
    pub fn with_context(context: ErrorContext, message: impl Into<String>) -> Self {
        MessageError::WithContext {
            context,
            message: message.into(),
        }
    }

    /// 创建通用错误
    pub fn other(msg: impl Into<String>) -> Self {
        MessageError::Other(msg.into())
    }

    /// 从 NeteaseError 创建
    pub fn from_netease(err: crate::error::NeteaseError) -> Self {
        MessageError::Netease(err.into())
    }

    /// 从 AudioError 创建
    #[allow(dead_code)]
    pub fn from_audio(err: crate::error::AudioError) -> Self {
        MessageError::Audio(err.into())
    }

    /// 从 AppError 创建
    #[allow(dead_code)]
    pub fn from_app(err: crate::error::AppError) -> Self {
        MessageError::App(err.into())
    }
}

// 实现 From traits 以便自动转换
impl From<crate::error::NeteaseError> for MessageError {
    fn from(err: crate::error::NeteaseError) -> Self {
        MessageError::Netease(err.into())
    }
}

impl From<crate::error::AudioError> for MessageError {
    fn from(err: crate::error::AudioError) -> Self {
        MessageError::Audio(err.into())
    }
}

impl From<crate::error::AppError> for MessageError {
    fn from(err: crate::error::AppError) -> Self {
        MessageError::App(err.into())
    }
}

impl std::fmt::Display for AppErrorVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppErrorVariant::Io(s) => write!(f, "IO 错误: {s}"),
            AppErrorVariant::Serde(s) => write!(f, "JSON 序列化失败: {s}"),
            AppErrorVariant::Settings(s) => write!(f, "设置错误: {s}"),
            AppErrorVariant::Netease(e) => write!(f, "网易云音乐错误: {e}"),
            AppErrorVariant::Audio(e) => write!(f, "音频错误: {e}"),
            AppErrorVariant::DataDir(s) => write!(f, "数据目录错误: {s}"),
            AppErrorVariant::Config(s) => write!(f, "配置错误: {s}"),
            AppErrorVariant::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::fmt::Display for NeteaseErrorVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NeteaseErrorVariant::Reqwest(s) => write!(f, "网络请求错误: {s}"),
            NeteaseErrorVariant::Io(s) => write!(f, "IO 错误: {s}"),
            NeteaseErrorVariant::Serde(s) => write!(f, "序列化错误: {s}"),
            NeteaseErrorVariant::Crypto(s) => write!(f, "加密错误: {s}"),
            NeteaseErrorVariant::CookieValidationFailed(s) => write!(f, "Cookie 验证失败: {s}"),
            NeteaseErrorVariant::Api { code, msg } => write!(f, "API 错误 (code={code}): {msg}"),
            NeteaseErrorVariant::BadHeader(s) => write!(f, "Header 构造失败: {s}"),
            NeteaseErrorVariant::BadInput(s) => write!(f, "输入错误: {s}"),
        }
    }
}

impl std::fmt::Display for AudioErrorVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioErrorVariant::OpenFile { title, source } => {
                write!(f, "打开音频文件失败({title}): {source}")
            }
            AudioErrorVariant::Decode { title, source } => {
                write!(f, "解码音频失败({title}): {source}")
            }
            AudioErrorVariant::Download(s) => write!(f, "下载失败: {s}"),
            AudioErrorVariant::Cache(s) => write!(f, "缓存操作失败: {s}"),
            AudioErrorVariant::Init(s) => write!(f, "播放器初始化失败: {s}"),
            AudioErrorVariant::Seek(s) => write!(f, "Seek 失败: {s}"),
            AudioErrorVariant::OutputStream(s) => write!(f, "创建音频输出流失败: {s}"),
            AudioErrorVariant::Sink(s) => write!(f, "创建 Sink 失败: {s}"),
            AudioErrorVariant::FileNotFound(s) => write!(f, "音频文件不存在: {s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_error_from_netease() {
        let netease_err = crate::error::NeteaseError::BadInput("测试参数");
        let msg_err = MessageError::from(netease_err);
        assert!(matches!(msg_err, MessageError::Netease(_)));
        assert!(msg_err.to_string().contains("输入错误"));
    }

    #[test]
    fn test_message_error_with_context() {
        let err = MessageError::with_context(ErrorContext::Login, "登录失败");
        assert!(matches!(err, MessageError::WithContext { .. }));
        assert!(err.to_string().contains("登录"));
        assert!(err.to_string().contains("登录失败"));
    }

    #[test]
    fn test_is_retryable() {
        // 网络错误可重试
        let netease_err = NeteaseErrorVariant::Reqwest("timeout".to_string());
        let msg_err = MessageError::Netease(netease_err);
        assert!(msg_err.is_retryable());

        // Crypto 错误不可重试
        let netease_err = NeteaseErrorVariant::Crypto("invalid key".to_string());
        let msg_err = MessageError::Netease(netease_err);
        assert!(!msg_err.is_retryable());
    }

    #[test]
    fn test_to_display_string() {
        let err = MessageError::Other("测试错误".to_string());
        assert_eq!(err.to_display_string(), "测试错误");
    }

    #[test]
    fn test_error_context_display() {
        assert_eq!(ErrorContext::Login.to_string(), "登录");
        assert_eq!(ErrorContext::Search.to_string(), "搜索");
        assert_eq!(ErrorContext::Playback.to_string(), "播放");
    }

    #[test]
    fn test_clone_message_error() {
        let err1 = MessageError::Other("测试".to_string());
        let err2 = err1.clone();
        assert_eq!(err1.to_string(), err2.to_string());
    }
}
