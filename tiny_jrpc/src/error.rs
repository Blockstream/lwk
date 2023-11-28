use std::{fmt::Display, io};

use crate::RpcError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Inner(#[from] InnerError),

    #[error(transparent)]
    Implementation(#[from] ImplementationDefinedError),

    #[error("Received stop command")]
    Stop,
}

#[derive(Debug, thiserror::Error)]
#[error("Implementation defined error({code}): {message}")]
pub struct ImplementationDefinedError {
    message: String,
    code: ImplementationDefinedCode,
    data: Option<serde_json::Value>,
}

/// Caller should not instantiate this, but rely on [`ImplementationDefinedError`] or the
/// [`Error::Stop`] variant
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InnerError {
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
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Implementation(ImplementationDefinedError {
            message,
            code: GENERIC,
            data: None,
        })
    }
}

impl Error {
    pub fn new_implementation_defined(
        e: &impl Display,
        code: ImplementationDefinedCode,
        data: Option<serde_json::Value>,
    ) -> Self {
        Error::Implementation(ImplementationDefinedError {
            message: e.to_string(),
            code,
            data,
        })
    }
}

pub(crate) trait AsRpcError {
    fn as_rpc_error(&self) -> RpcError;
}

impl AsRpcError for InnerError {
    fn as_rpc_error(&self) -> RpcError {
        let (code, data) = match self {
            InnerError::Io(_) => (IO_ERROR, None),
            InnerError::Serde(_) => (PARSE_ERROR, None),
            InnerError::NoContentType => (NO_CONTENT_TYPE, None),
            InnerError::WrongContentType => (WRONG_CONTENT_TYPE, None),
            InnerError::ReservedMethodPrefix => (METHOD_RESERVED, None),
            InnerError::InvalidVersion => (INVALID_VERSION, None),
        };

        RpcError {
            code,
            message: self.to_string(),
            data,
        }
    }
}

impl AsRpcError for ImplementationDefinedError {
    fn as_rpc_error(&self) -> RpcError {
        RpcError {
            code: self.code.0,
            message: self.message.clone(),
            data: self.data.clone(),
        }
    }
}

impl AsRpcError for Error {
    fn as_rpc_error(&self) -> RpcError {
        match self {
            Error::Inner(e) => e.as_rpc_error(),
            Error::Implementation(e) => e.as_rpc_error(),
            Error::Stop => RpcError {
                code: STOP_ERROR,
                message: "Server stopped".to_string(),
                data: None,
            },
        }
    }
}

// from: https://www.jsonrpc.org/specification#error_object

// -32700 	Parse error 	Invalid JSON was received by the server.  An error occurred on the server while parsing the JSON text.
const PARSE_ERROR: i64 = -32_700;

// -32600 	Invalid Request 	The JSON sent is not a valid Request object.
// const INVALID_REQUEST: i64 = -32_600; // TODO if failing to parse the request object, try to parse as Value and if succesfull return this instead of PARSE_ERROR

// -32601 	Method not found 	The method does not exist / is not available.
pub(crate) const METHOD_NOT_FOUND: i64 = -32_601;

// -32602 	Invalid params 	Invalid method parameter(s).
// const INVALID_PARAMS: i64 = -32_602;

// -32603 	Internal error 	Internal JSON-RPC error.
// const INTERNAL_ERROR: i64 = -32_603;

// -32000 to -32099 	Server error 	Reserved for implementation-defined server-errors.
const IO_ERROR: i64 = -32_000;
const NO_CONTENT_TYPE: i64 = -32_001;
const WRONG_CONTENT_TYPE: i64 = -32_002;
const METHOD_RESERVED: i64 = -32_003;
const INVALID_VERSION: i64 = -32_004;

// GENERIC = -32_098, // TODO remove
const STOP_ERROR: i64 = -32_099;

#[derive(Debug)]
pub struct ImplementationDefinedCode(i64);
impl ImplementationDefinedCode {
    pub const fn new(val: i64) -> Option<Self> {
        if val > -32004 || val < -32099 {
            None
        } else {
            Some(Self(val))
        }
    }
}
pub const GENERIC: ImplementationDefinedCode = ImplementationDefinedCode(-32_005);
impl From<ImplementationDefinedCode> for i64 {
    fn from(value: ImplementationDefinedCode) -> Self {
        value.0
    }
}
impl From<&ImplementationDefinedCode> for i64 {
    fn from(value: &ImplementationDefinedCode) -> Self {
        value.0
    }
}
impl Display for ImplementationDefinedCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
