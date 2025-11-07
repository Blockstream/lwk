use crate::SwapState;
use boltz_client::error::Error as BoltzError;
use boltz_client::lightning_invoice::ParseOrSemanticError;
use lightning::bitcoin::XKeyIdentifier;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid swap state: {0}")]
    InvalidSwapState(String),

    #[error("Invalid bolt11 invoice: {0}")]
    InvalidBolt11Invoice(ParseOrSemanticError),

    #[error("Boltz API error: {0}")]
    BoltzApi(BoltzError),

    #[error("Receiver error: {0}")]
    Receiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("TryReceiver error: {0}")]
    TryReceiver(#[from] tokio::sync::broadcast::error::TryRecvError),

    #[error("Unexpected status {status} for swap {swap_id}. Last state: {last_state}")]
    UnexpectedUpdate {
        swap_id: String,
        status: String,
        last_state: SwapState,
    },

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

    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("BIP32 derivation error: {0}")]
    Bip32(#[from] lwk_wollet::bitcoin::bip32::Error),

    #[error("Secp256k1 error: {0}")]
    Secp256k1(#[from] lwk_wollet::secp256k1::Error),

    #[error("Swap {swap_id} has expired with status {status}")]
    Expired { swap_id: String, status: String },

    #[error("Swap restoration error: {0}")]
    SwapRestoration(String),

    #[error("Broadcast failed after retrying")]
    RetryBroadcastFailed,

    #[error("Bolt12 (offers) are not yet supported")]
    Bolt12Unsupported,

    #[error("Mnemonic identifier mismatch: {0} != {1}")]
    MnemonicIdentifierMismatch(XKeyIdentifier, XKeyIdentifier),

    #[error("No update available, continuing polling")]
    NoBoltzUpdate,
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
