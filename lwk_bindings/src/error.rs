use std::array::TryFromSliceError;
use std::sync::{MutexGuard, PoisonError};

use elements::pset::ParseError;

/// Possible errors emitted
#[derive(uniffi::Error, thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum LwkError {
    #[error("{msg}")]
    Generic { msg: String },

    #[error("Poison error: {msg}")]
    PoisonError { msg: String },

    #[error("Invoice contain a magic routing hint, there is no need to pay via Boltz, pay directly to: {uri}")]
    MagicRoutingHint {
        address: String,
        amount: u64,
        uri: String,
    },

    #[error("Swap {swap_id} has expired with status {status}")]
    SwapExpired { swap_id: String, status: String },

    #[error("There are no message to receive on the boltz web socket, continuing polling")]
    NoBoltzUpdate,

    #[error("Calling a function on an object that has already been consumed, like for example calling complete() on object that already is completed")]
    ObjectConsumed,
}

impl From<lwk_wollet::Error> for LwkError {
    fn from(value: lwk_wollet::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<ParseError> for LwkError {
    fn from(value: ParseError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::pset::Error> for LwkError {
    fn from(value: elements::pset::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::encode::Error> for LwkError {
    fn from(value: elements::encode::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::transaction::ParseOutPointError> for LwkError {
    fn from(value: elements::bitcoin::transaction::ParseOutPointError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::key::FromWifError> for LwkError {
    fn from(value: elements::bitcoin::key::FromWifError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::hashes::hex::HexToBytesError> for LwkError {
    fn from(value: elements::hashes::hex::HexToBytesError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::hashes::hex::HexToArrayError> for LwkError {
    fn from(value: elements::hashes::hex::HexToArrayError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::AddressError> for LwkError {
    fn from(value: elements::AddressError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<lwk_signer::bip39::Error> for LwkError {
    fn from(value: lwk_signer::bip39::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<lwk_signer::NewError> for LwkError {
    fn from(value: lwk_signer::NewError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<lwk_signer::SignError> for LwkError {
    fn from(value: lwk_signer::SignError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<serde_json::Error> for LwkError {
    fn from(value: serde_json::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<lwk_common::QrError> for LwkError {
    fn from(value: lwk_common::QrError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
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
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::secp256k1::Error> for LwkError {
    fn from(value: elements::bitcoin::secp256k1::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::key::FromSliceError> for LwkError {
    fn from(value: elements::bitcoin::key::FromSliceError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::key::ParsePublicKeyError> for LwkError {
    fn from(value: elements::bitcoin::key::ParsePublicKeyError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::secp256k1_zkp::Error> for LwkError {
    fn from(value: elements::secp256k1_zkp::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::bip32::Error> for LwkError {
    fn from(value: elements::bitcoin::bip32::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::locktime::Error> for LwkError {
    fn from(value: elements::locktime::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::VerificationError> for LwkError {
    fn from(value: elements::VerificationError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

#[cfg(feature = "simplicity")]
impl From<lwk_simplicity_options::error::ProgramError> for LwkError {
    fn from(value: lwk_simplicity_options::error::ProgramError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::UnblindError> for LwkError {
    fn from(value: elements::UnblindError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<lwk_wollet::elements_miniscript::psbt::Error> for LwkError {
    fn from(value: lwk_wollet::elements_miniscript::psbt::Error) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<TryFromSliceError> for LwkError {
    fn from(value: TryFromSliceError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

impl From<elements::bitcoin::address::ParseError> for LwkError {
    fn from(value: elements::bitcoin::address::ParseError) -> Self {
        LwkError::Generic {
            msg: format!("{value:?}"),
        }
    }
}

#[cfg(feature = "lightning")]
impl From<lwk_boltz::Error> for LwkError {
    fn from(value: lwk_boltz::Error) -> Self {
        match value {
            lwk_boltz::Error::MagicRoutingHint {
                address,
                amount,
                uri,
            } => LwkError::MagicRoutingHint {
                address,
                amount,
                uri,
            },
            lwk_boltz::Error::Expired { swap_id, status } => {
                LwkError::SwapExpired { swap_id, status }
            }
            lwk_boltz::Error::NoBoltzUpdate => LwkError::NoBoltzUpdate,
            _ => LwkError::Generic {
                msg: format!("{value:?}"),
            },
        }
    }
}
