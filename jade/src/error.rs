use std::time::SystemTimeError;

use ciborium::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Jade Error: {0}")]
    JadeError(ErrorDetails),
    #[error("IO Error: {0}")]
    IoError(std::io::Error),
    #[error("SystemTime Error: {0}")]
    SystemTimeError(SystemTimeError),
    #[cfg(feature = "serial")]
    #[error("Serial Error: {0}")]
    SerialError(#[from] serialport::Error),

    #[error("Jade returned neither an error nor a result")]
    JadeNeitherErrorNorResult,

    #[error(transparent)]
    Ser(#[from] ciborium::ser::Error<std::io::Error>),

    #[error(transparent)]
    Des(#[from] ciborium::de::Error<std::io::Error>),

    #[error(transparent)]
    Bip32(#[from] elements::bitcoin::bip32::Error),

    #[error("Mismatching network, jade was initialized with: {init} but the method params received {passed}")]
    MismatchingXpub {
        init: crate::Network,
        passed: crate::Network,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorDetails {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl std::fmt::Display for ErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error code: {} - message: {}", self.code, self.message)
    }
}
