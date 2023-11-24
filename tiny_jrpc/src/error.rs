use std::{fmt::Display, io};

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

    #[error("Implementation defined error({code}): {message}")]
    ImplementationDefined {
        message: String,
        code: ImplementationDefinedCode,
        data: Option<serde_json::Value>,
    },

    #[error("Received stop command")]
    Stop,
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::ImplementationDefined {
            message,
            code: GENERIC,
            data: None,
        }
    }
}

impl Error {
    pub fn new_implementation_defined(e: impl Display, code: ImplementationDefinedCode) -> Self {
        Error::ImplementationDefined {
            message: e.to_string(),
            code,
            data: None,
        }
    }

    pub fn as_rpc_error(&self) -> RpcError {
        let (code, data) = match self {
            Error::Io(_) => (IO_ERROR, None),
            Error::Serde(_) => (PARSE_ERROR, None),
            Error::NoContentType => (NO_CONTENT_TYPE, None),
            Error::WrongContentType => (WRONG_CONTENT_TYPE, None),
            Error::ReservedMethodPrefix => (METHOD_RESERVED, None),
            Error::InvalidVersion => (INVALID_VERSION, None),
            Error::ImplementationDefined {
                message: _,
                code,
                data,
            } => (code.into(), data.clone()),
            Error::Stop => (STOP_ERROR, None),
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
        if val > -32000 || val < -32999 {
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
