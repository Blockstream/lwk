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

    #[error("Lending offer error: {0}")]
    LendingOfferError(#[from] lending_contracts::programs::lending::LendingOfferError),

    #[error("Blinding error: {0}")]
    BlindingError(#[from] lwk_wollet::elements::pset::PsetBlindError),

    #[error("Cannot liquidate offer: current height {current_height}, but offer can be liquidated after {loan_expiration_height}")]
    CannotLiquidate {
        current_height: u32,
        loan_expiration_height: u32,
    },
}
