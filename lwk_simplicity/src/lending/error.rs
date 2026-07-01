use crate::error::ProgramError;

#[derive(thiserror::Error, Debug)]
pub enum LendingError {
    #[error("Program error: {0}")]
    Program(#[from] ProgramError),

    #[error("Wallet error: {0}")]
    Wallet(#[from] lwk_wollet::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
