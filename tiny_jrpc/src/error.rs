use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
    #[error("Serde JSON Error: {0}")]
    Serde(#[from] serde_json::Error),
}
