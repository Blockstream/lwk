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

impl From<lwk_wollet::Error> for LwkError {
    fn from(value: lwk_wollet::Error) -> Self {
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

impl From<elements::hashes::hex::HexToBytesError> for LwkError {
    fn from(value: elements::hashes::hex::HexToBytesError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::hashes::hex::HexToArrayError> for LwkError {
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

impl From<lwk_signer::bip39::Error> for LwkError {
    fn from(value: lwk_signer::bip39::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<lwk_signer::NewError> for LwkError {
    fn from(value: lwk_signer::NewError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<lwk_signer::SignError> for LwkError {
    fn from(value: lwk_signer::SignError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<serde_json::Error> for LwkError {
    fn from(value: serde_json::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<lwk_common::QrError> for LwkError {
    fn from(value: lwk_common::QrError) -> Self {
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

impl From<&str> for LwkError {
    fn from(msg: &str) -> Self {
        LwkError::Generic {
            msg: msg.to_owned(),
        }
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for LwkError {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        LwkError::PoisonError { msg: e.to_string() }
    }
}

impl From<lwk_common::precision::Error> for LwkError {
    fn from(value: lwk_common::precision::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::bitcoin::secp256k1::Error> for LwkError {
    fn from(value: elements::bitcoin::secp256k1::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::UnblindError> for LwkError {
    fn from(value: elements::UnblindError) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<lwk_wollet::elements_miniscript::psbt::Error> for LwkError {
    fn from(value: lwk_wollet::elements_miniscript::psbt::Error) -> Self {
        LwkError::Generic {
            msg: format!("{:?}", value),
        }
    }
}
