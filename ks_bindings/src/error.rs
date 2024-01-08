use std::sync::{MutexGuard, PoisonError};

use elements::pset::ParseError;

/// Possible errors emitted
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum Error {
    #[error("{msg}")]
    Generic { msg: String },

    #[error("Poison error: {msg}")]
    PoisonError { msg: String },
}

impl From<wollet::Error> for Error {
    fn from(value: wollet::Error) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::pset::Error> for Error {
    fn from(value: elements::pset::Error) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::encode::Error> for Error {
    fn from(value: elements::encode::Error) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::bitcoin::transaction::ParseOutPointError> for Error {
    fn from(value: elements::bitcoin::transaction::ParseOutPointError) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::hashes::hex::Error> for Error {
    fn from(value: elements::hashes::hex::Error) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl From<elements::AddressError> for Error {
    fn from(value: elements::AddressError) -> Self {
        Error::Generic {
            msg: format!("{:?}", value),
        }
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for Error {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        Error::PoisonError { msg: e.to_string() }
    }
}
