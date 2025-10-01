use boltz_client::error::Error as BoltzError;
use boltz_client::lightning_invoice::ParseOrSemanticError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid bolt11 invoice: {0}")]
    InvalidBolt11Invoice(ParseOrSemanticError),

    #[error("Boltz API error: {0}")]
    BoltzApi(BoltzError),
}

impl From<BoltzError> for Error {
    fn from(err: BoltzError) -> Self {
        Error::BoltzApi(err)
    }
}

impl From<ParseOrSemanticError> for Error {
    fn from(err: ParseOrSemanticError) -> Self {
        Error::InvalidBolt11Invoice(err)
    }
}
