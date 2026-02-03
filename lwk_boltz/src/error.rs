use crate::SwapAsset;
use crate::SwapState;
use boltz_client::elements::AddressError;
use boltz_client::error::Error as BoltzError;
use boltz_client::lightning_invoice::ParseOrSemanticError;
use lightning::bitcoin::XKeyIdentifier;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to build refund transaction")]
    FailBuildingRefundTransaction,

    #[error("Invalid electrum url: {0}")]
    InvalidElectrumUrl(#[from] lwk_wollet::UrlError),

    #[error("Invalid swap state: {0}")]
    InvalidSwapState(String),

    #[error("Invalid bolt11 invoice: {0}")]
    InvalidBolt11Invoice(ParseOrSemanticError),

    #[error("Boltz API error: {0}")]
    BoltzApi(BoltzError),

    #[error("Elements address error: {0}")]
    ElementsAddressError(#[from] AddressError),

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

    #[error("LnUrl are not supperted")]
    LnUrlUnsupported,

    #[error("Mnemonic identifier mismatch: {0} != {1}")]
    MnemonicIdentifierMismatch(XKeyIdentifier, XKeyIdentifier),

    #[error("No update available, continuing polling")]
    NoBoltzUpdate,

    #[error("Invalid swap pair: {from:?} -> {to:?}")]
    InvalidSwapPair { from: SwapAsset, to: SwapAsset },

    #[error("Quote builder missing {0} parameter")]
    MissingQuoteParam(&'static str),

    #[error("Swap pair not available from Boltz API")]
    PairNotAvailable,

    #[error("Internal lock error: {0}")]
    LockPoisoned(String),

    #[error("Store error: {0}")]
    Store(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Store not configured")]
    StoreNotConfigured,

    #[error("Encryption error: {0}")]
    Encryption(String),
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

impl From<aes_gcm_siv::Error> for Error {
    fn from(err: aes_gcm_siv::Error) -> Self {
        Error::Encryption(err.to_string())
    }
}
