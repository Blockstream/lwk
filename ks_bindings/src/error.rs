use std::sync::{MutexGuard, PoisonError};

use elements::pset::ParseError;

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

impl<T> From<PoisonError<MutexGuard<'_, T>>> for Error {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        Error::PoisonError { msg: e.to_string() }
    }
}
