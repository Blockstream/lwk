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
        use RpcIntErrors::*;

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
            code: code as i64,
            message: self.to_string(),
            data,
        }
    }
}

// from: https://www.jsonrpc.org/specification#error_object

#[allow(non_camel_case_types)]
pub enum RpcIntErrors {
    // -32700 	Parse error 	Invalid JSON was received by the server.  An error occurred on the server while parsing the JSON text.
    PARSE_ERROR = -32_700,

    // -32600 	Invalid Request 	The JSON sent is not a valid Request object.
    INVALID_REQUEST = -32_600,

    // -32601 	Method not found 	The method does not exist / is not available.
    METHOD_NOT_FOUND = -32_601,

    // -32602 	Invalid params 	Invalid method parameter(s).
    INVALID_PARAMS = -32_602,

    // -32603 	Internal error 	Internal JSON-RPC error.
    INTERNAL_ERROR = -32_603,

    // -32000 to -32099 	Server error 	Reserved for implementation-defined server-errors.
    IO_ERROR = -32_000,
    NO_CONTENT_TYPE = -32_001,
    WRONG_CONTENT_TYPE = -32_002,
    METHOD_RESERVED = -32_003,
    INVALID_VERSION = -32_004,
    WOLLET_ERROR = -32_005,
    SIGNER_NEW_ERROR = -32_006,
    SIGNER_ERROR = -32_007,
    WALLET_NOT_EXIST_ERROR = -32_008,
    WALLET_ALREADY_LOADED = -32_009,
    SIGNER_NOT_EXIST_ERROR = -32_010,
    SIGNER_ALREADY_LOADED = -32_011,

    STOP_ERROR = -32_099,
}
