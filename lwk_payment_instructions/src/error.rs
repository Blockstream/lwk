use crate::PaymentKind;

/// Error type for the whole crate.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Unexpected payment kind.
    #[error("Expected payment kind {0:?}")]
    ExpectedKind(PaymentKind),

    /// Liquid address network does not match the requested schema.
    #[error(
        "Wrong Liquid address network, expected {expected} address",
        expected = if *.expected_mainnet { "mainnet" } else { "testnet" }
    )]
    WrongLiquidNetwork { expected_mainnet: bool },

    /// BIP353 did not resolve to a lightning offer.
    #[error("BIP353 did not resolve to a lightning offer")]
    Bip353OfferNotFound,

    /// Invalid URI schema.
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),

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
