use std::time::SystemTimeError;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Jade Error: {0}")]
    JadeError(ErrorDetails),
    #[error("IO Error: {0}")]
    IoError(std::io::Error),
    #[error("SystemTime Error: {0}")]
    SystemTimeError(SystemTimeError),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorDetails {
    code: i64,
    message: String,
    data: Vec<u8>,
}

impl std::fmt::Display for ErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error code: {} - message: {}", self.code, self.message)
    }
}
