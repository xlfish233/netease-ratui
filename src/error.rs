#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Netease(#[from] crate::netease::NeteaseError),

    #[error("{0}")]
    Other(String),
}
