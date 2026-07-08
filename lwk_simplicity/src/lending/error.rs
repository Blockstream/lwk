use crate::error::ProgramError;

#[derive(thiserror::Error, Debug)]
pub enum LendingError {
    #[error("Program error: {0}")]
    Program(#[from] ProgramError),

    #[error("Wallet error: {0}")]
    Wallet(#[from] lwk_wollet::Error),

    #[error("Indexer client error: {0}")]
    IndexerClient(#[from] crate::lending::indexer::client::ClientError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Generic error: {0}")]
    Generic(String),

    #[error("Cannot parse factory data: {0}")]
    CannotParseFactory(String),
}
