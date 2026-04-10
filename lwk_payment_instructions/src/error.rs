use crate::PaymentKind;

/// Error type for the whole crate.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Unexpected payment kind.
    #[error("Expected payment kind {0:?}")]
    ExpectedKind(PaymentKind),

    /// Generic error.
    #[error("{0}")]
    Generic(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}
