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
}

impl Error {
    pub fn as_rpc_error(&self) -> RpcError {
        let (code, message, data) = match self {
            Error::Io(_) => (IO_ERROR, "An IO error occurred.".to_string(), None),
            Error::Serde(_) => (
                PARSE_ERROR,
                "Invalid JSON was received by the server.".to_string(),
                None,
            ),
            Error::NoContentType => (
                NO_CONTENT_TYPE,
                "Content-Type header is missing.".to_string(),
                None,
            ),
            Error::WrongContentType => (
                WRONG_CONTENT_TYPE,
                "Content-Type header is invalid, it should be 'application/json'.".to_string(),
                None,
            ),
            Error::ReservedMethodPrefix => (
                METHOD_RESERVED,
                "Method names that begin with 'rpc.' are reserved for system extensions."
                    .to_string(),
                None,
            ),
            Error::InvalidVersion => (
                INVALID_VERSION,
                "jsonrpc version is invalid, it should be '2.0'.".to_string(),
                None,
            ),
            Error::Wollet(_) => (WOLLET_ERROR, "Watch Only wallet error.".to_string(), None),
        };

        RpcError {
            code,
            message,
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
