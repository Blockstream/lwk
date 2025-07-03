use std::sync::{MutexGuard, PoisonError};

use lwk_tiny_jrpc::error::ImplementationDefinedCode;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Tiny HTTP Error: {0}")]
    TinyHttp(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("JSON RPC HTTP Client Error: {0}")]
    JsonRpcHttp(#[from] jsonrpc::simple_http::Error),

    #[error("JSON RPC Client Error: {0}")]
    JsonRpcClient(#[from] jsonrpc::Error),

    #[error("Serde JSON Error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Jade Error: {0}")]
    Jade(#[from] lwk_jade::Error),

    #[error("Wollet Error: {0}")]
    Wollet(#[from] lwk_wollet::Error),

    #[error("Address Error: {0}")]
    Address(#[from] lwk_wollet::elements::AddressError),

    #[error("HexArray Error: {0}")]
    HexArray(#[from] lwk_wollet::hashes::hex::HexToArrayError),

    #[error("Reqwest Error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Hex Error: {0}")]
    Hex(lwk_wollet::elements::hex::Error),

    #[error("Elements Encode Error: {0}")]
    ElementsEncode(lwk_wollet::elements::encode::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Trying to start an already started server")]
    AlreadyStarted,

    #[error("Trying to join a non started server")]
    NotStarted,

    #[error("In the response received neither the result nor the error are set")]
    NeitherResultNorErrorSet,

    #[error("Rpc returned an error {0:?}")]
    RpcError(jsonrpc::error::RpcError),

    #[error("Signer New Error: {0}")]
    SignerNew(#[from] lwk_signer::NewError),

    #[error("Signer Error: {0}")]
    Signer(#[from] lwk_signer::SignerError),

    #[error(transparent)]
    QrError(#[from] lwk_common::QrError),

    #[error("Wallet '{0}' does not exist")]
    WalletNotExist(String),

    #[error("Wallet '{0}' is already loaded")]
    WalletAlreadyLoaded(String),

    #[error("Tx '{0}' was not found in wallet '{1}'")]
    WalletTxNotFound(String, String),

    #[error("Signer '{0}' does not exist")]
    SignerNotExist(String),

    #[error("Signer '{0}' is already loaded")]
    SignerAlreadyLoaded(String),

    #[error("Asset '{0}' does not exist")]
    AssetNotExist(String),

    #[error("Asset '{0}' already inserted")]
    AssetAlreadyInserted(String),

    #[error(transparent)]
    MethodNotExist(#[from] crate::method::MethodNotExist),

    #[error("Poison error: {0}")]
    PoisonError(String),

    #[error("Feature \"serial\" is disabled, enable it to solve this error")]
    FeatSerialDisabled,

    #[error("Cannot start the server at \"{0}\". It is probably already running.")]
    ServerStart(String),

    #[error("Error re-applying start request\nError {0}\nAt line {1} from file {2}\nConsider: (a) correct the line (b) remove the line (c) remove the file")]
    StartStateLoad(String, usize, String),

    #[error("Received stop command")]
    Stop,

    // TODO remove into specific errors
    #[error("Generic error {0}")]
    Generic(String),
}

impl Error {
    /// Return error codes, no different variants should return the same value
    pub fn as_impl_defined_code(&self) -> ImplementationDefinedCode {
        match self {
            Error::Jade(_) => ImplementationDefinedCode::new(-32_013).expect("static"),
            Error::Wollet(_) => ImplementationDefinedCode::new(-32_005).expect("static"),
            Error::SignerNew(_) => ImplementationDefinedCode::new(-32_006).expect("static"),
            Error::Signer(_) => ImplementationDefinedCode::new(-32_007).expect("static"),
            Error::WalletNotExist(_) => ImplementationDefinedCode::new(-32_008).expect("static"),
            Error::WalletAlreadyLoaded(_) => {
                ImplementationDefinedCode::new(-32_009).expect("static")
            }
            Error::SignerNotExist(_) => ImplementationDefinedCode::new(-32_010).expect("static"),
            Error::SignerAlreadyLoaded(_) => {
                ImplementationDefinedCode::new(-32_011).expect("static")
            }

            _ => lwk_tiny_jrpc::error::GENERIC,
        }
    }

    /// Used to create error as structured data, easily parsable by the caller
    pub fn as_error_value(&self) -> Option<serde_json::Value> {
        match self {
            Error::WalletNotExist(n) => Some(json!({"name": n.to_string()})),
            Error::SignerNotExist(n) => Some(json!({"name": n.to_string()})),
            _ => None,
        }
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Generic(message)
    }
}
impl From<lwk_wollet::elements::hex::Error> for Error {
    fn from(value: lwk_wollet::elements::hex::Error) -> Self {
        Error::Hex(value)
    }
}
impl From<lwk_wollet::elements::encode::Error> for Error {
    fn from(value: lwk_wollet::elements::encode::Error) -> Self {
        Error::ElementsEncode(value)
    }
}

impl From<Error> for lwk_tiny_jrpc::error::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Stop => lwk_tiny_jrpc::error::Error::Stop,
            e => lwk_tiny_jrpc::error::Error::new_implementation_defined(
                &e,
                e.as_impl_defined_code(),
                e.as_error_value(),
            ),
        }
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for Error {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        Error::PoisonError(e.to_string())
    }
}
