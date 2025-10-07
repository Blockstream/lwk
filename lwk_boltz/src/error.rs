use boltz_client::error::Error as BoltzError;
use boltz_client::lightning_invoice::ParseOrSemanticError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid bolt11 invoice: {0}")]
    InvalidBolt11Invoice(ParseOrSemanticError),

    #[error("Boltz API error: {0}")]
    BoltzApi(BoltzError),

    #[error("Receiver error: {0}")]
    Receiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Unexpected status {status} for swap {swap_id}")]
    UnexpectedUpdate { swap_id: String, status: String },

    #[error("Invoice without amount {0}")]
    InvoiceWithoutAmount(String),

    #[error("Expected amount {0} is lower than amount in invoice {1}")]
    ExpectedAmountLowerThanInvoice(u64, String),

    #[error("Missing invoice in response for swap id {0}")]
    MissingInvoiceInResponse(String),

    #[error("Magic routing hint not supported for now. Swap id {0}")]
    InvoiceWithoutMagicRoutingHint(String),

    #[error("Timeout waiting for swap update for swap {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invoice contain a magic routing hint, there is no need to pay via Boltz, pay directly to: {uri}")]
    MagicRoutingHint {
        address: String,
        amount: u64,
        uri: String,
    },
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
