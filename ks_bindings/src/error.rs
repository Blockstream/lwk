use std::sync::{MutexGuard, PoisonError};

use elements::pset::ParseError;

/// Possible errors emitted
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum LwkError {
    #[error("{msg}")]
    Generic { msg: String },

    #[error("Poison error: {msg}")]
    PoisonError { msg: String },
}

impl From<wollet::Error> for LwkError {
    fn from(value: wollet::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<ParseError> for LwkError {
    fn from(value: ParseError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::pset::Error> for LwkError {
    fn from(value: elements::pset::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::encode::Error> for LwkError {
    fn from(value: elements::encode::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::bitcoin::transaction::ParseOutPointError> for LwkError {
    fn from(value: elements::bitcoin::transaction::ParseOutPointError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::hashes::hex::HexToBytesError> for Error {
    fn from(value: elements::hashes::hex::HexToBytesError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::hashes::hex::HexToArrayError> for Error {
    fn from(value: elements::hashes::hex::HexToArrayError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::AddressError> for LwkError {
    fn from(value: elements::AddressError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<signer::bip39::Error> for LwkError {
    fn from(value: signer::bip39::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<signer::NewError> for LwkError {
    fn from(value: signer::NewError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<signer::SignError> for LwkError {
    fn from(value: signer::SignError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<String> for LwkError {
    fn from(msg: String) -> Self {
        LwkError::Generic { msg }
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for LwkError {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        LwkError::PoisonError { msg: e.to_string() }
    }
}
