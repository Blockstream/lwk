use boltz_client::lightning_invoice::ParseOrSemanticError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid bolt11 invoice: {0}")]
    InvalidBolt11Invoice(ParseOrSemanticError),
}

impl From<ParseOrSemanticError> for Error {
    fn from(err: ParseOrSemanticError) -> Self {
        Error::InvalidBolt11Invoice(err)
    }
}
