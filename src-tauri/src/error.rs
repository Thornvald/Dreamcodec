use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "type")]
pub enum AppError {
    #[error("I/O Error: {0}")]
    Io(String),

    #[error("FFmpeg Error: {0}")]
    Ffmpeg(String),

    #[error("Tauri Error: {0}")]
    Tauri(String),

    #[error("Internal Error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::Tauri(err.to_string())
    }
}
