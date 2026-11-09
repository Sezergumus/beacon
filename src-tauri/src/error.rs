use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("IO Error: {0}")]
    Io(String),

    #[error("Network Error: {0}")]
    Network(String),

    #[error("JVM Engine Error: {0}")]
    Jvm(String),

    #[error("Server Process Error: {0}")]
    Process(String),

    #[error("NBT Parse Error: {0}")]
    Nbt(String),

    #[error("Modrinth / Content Error: {0}")]
    Content(String),

    #[error("P2P Tunnel / DHT Error: {0}")]
    P2p(String),

    #[error("Host Migration Error: {0}")]
    Migration(String),

    #[error("{0}")]
    Custom(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Custom(format!("JSON serialization error: {}", err))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Custom(format!("{:#}", err))
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::Io(format!("Zip archive error: {}", err))
    }
}

impl From<fastnbt::error::Error> for AppError {
    fn from(err: fastnbt::error::Error) -> Self {
        AppError::Nbt(err.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
