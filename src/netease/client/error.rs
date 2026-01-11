use crate::netease::crypto;

#[derive(thiserror::Error, Debug)]
pub enum NeteaseError {
    #[error("reqwest 错误: {0}")]
    Reqwest(reqwest::Error),
    #[error("IO 错误: {0}")]
    Io(std::io::Error),
    #[error("serde 错误: {0}")]
    Serde(serde_json::Error),
    #[error("crypto 错误: {0}")]
    Crypto(crypto::CryptoError),
    #[error("Header 构造失败: {0}")]
    BadHeader(String),
    #[error("输入错误: {0}")]
    BadInput(&'static str),
    #[error("Cookie 验证失败: {0}")]
    CookieValidationFailed(String),
}
