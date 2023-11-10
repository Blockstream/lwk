use std::io;

use crate::RpcError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
    #[error("Serde JSON Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Request is missing Content-Type Header")]
    NoContentType,
    #[error("Request Content-Type is not specified as application/json")]
    WrongContentType,
    #[error("Reserved method prefix 'rpc.'")]
    ReservedMethodPrefix,
    #[error("'jsonrpc' version should be '2.0'")]
    InvalidVersion,
    #[error("Wollet Error: {0}")]
    Wollet(#[from] wollet::Error),
    #[error("Signer New Error: {0}")]
    SignerNew(#[from] signer::NewError),
    #[error("Signer Error: {0}")]
    Signer(#[from] signer::SignerError),
    #[error("Received stop command")]
    Stop,

    #[error("Wallet '{0}' does not exist")]
    WalletNotExist(String),

    #[error("Wallet '{0}' is already loaded")]
    WalletAlreadyLoaded(String),

    #[error("Signer '{0}' does not exist")]
    SignerNotExist(String),

    #[error("Signer '{0}' is already loaded")]
    SignerAlreadyLoaded(String),
}

impl Error {
    pub fn as_rpc_error(&self) -> RpcError {
        let (code, data) = match self {
            Error::Io(_) => (IO_ERROR, None),
            Error::Serde(_) => (PARSE_ERROR, None),
            Error::NoContentType => (NO_CONTENT_TYPE, None),
            Error::WrongContentType => (WRONG_CONTENT_TYPE, None),
            Error::ReservedMethodPrefix => (METHOD_RESERVED, None),
            Error::InvalidVersion => (INVALID_VERSION, None),
            Error::Wollet(_) => (WOLLET_ERROR, None),
            Error::SignerNew(_) => (SIGNER_NEW_ERROR, None),
            Error::Signer(_) => (SIGNER_ERROR, None),
            Error::Stop => (STOP_ERROR, None),
            Error::WalletNotExist(_) => (WALLET_NOT_EXIST_ERROR, None),
            Error::WalletAlreadyLoaded(_) => (WALLET_ALREADY_LOADED, None),
            Error::SignerAlreadyLoaded(_) => (SIGNER_ALREADY_LOADED, None),
            Error::SignerNotExist(_) => (SIGNER_NOT_EXIST_ERROR, None),
        };

        RpcError {
            code,
            message: self.to_string(),
            data,
        }
    }
}

// from: https://www.jsonrpc.org/specification#error_object

// -32700 	Parse error 	Invalid JSON was received by the server.  An error occurred on the server while parsing the JSON text.
pub const PARSE_ERROR: i64 = -32_700;

// -32600 	Invalid Request 	The JSON sent is not a valid Request object.
pub const INVALID_REQUEST: i64 = -32_600;

// -32601 	Method not found 	The method does not exist / is not available.
pub const METHOD_NOT_FOUND: i64 = -32_601;

// -32602 	Invalid params 	Invalid method parameter(s).
pub const INVALID_PARAMS: i64 = -32_602;

// -32603 	Internal error 	Internal JSON-RPC error.
pub const INTERNAL_ERROR: i64 = -32_603;

// -32000 to -32099 	Server error 	Reserved for implementation-defined server-errors.
pub const IO_ERROR: i64 = -32_000;
pub const NO_CONTENT_TYPE: i64 = -32_001;
pub const WRONG_CONTENT_TYPE: i64 = -32_002;
pub const METHOD_RESERVED: i64 = -32_003;
pub const INVALID_VERSION: i64 = -32_004;
pub const WOLLET_ERROR: i64 = -32_005;
pub const SIGNER_NEW_ERROR: i64 = -32_006;
pub const SIGNER_ERROR: i64 = -32_007;
pub const WALLET_NOT_EXIST_ERROR: i64 = -32_008;
pub const WALLET_ALREADY_LOADED: i64 = -32_009;
pub const SIGNER_NOT_EXIST_ERROR: i64 = -32_010;
pub const SIGNER_ALREADY_LOADED: i64 = -32_011;

pub const STOP_ERROR: i64 = -32_099;
