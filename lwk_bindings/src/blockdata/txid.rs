//! Liquid transaction identifier

use std::{fmt::Display, str::FromStr, sync::Arc};

use elements::hashes::Hash;

use crate::{types::Hex, LwkError};

/// A transaction identifier.
#[derive(uniffi::Object, PartialEq, Eq, Debug)]
#[uniffi::export(Display)]
pub struct Txid {
    inner: elements::Txid,
}

impl From<elements::Txid> for Txid {
    fn from(inner: elements::Txid) -> Self {
        Txid { inner }
    }
}

impl From<Txid> for elements::Txid {
    fn from(value: Txid) -> Self {
        value.inner
    }
}

impl From<&Txid> for elements::Txid {
    fn from(value: &Txid) -> Self {
        value.inner
    }
}

//use elements::bitcoin::hex::HexToArrayError;

impl FromStr for Txid {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(elements::Txid::from_str(s)?.into())
    }
}

impl Display for Txid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Txid {
    /// Construct a Txid object
    #[uniffi::constructor]
    pub fn new(hex: &Hex) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Txid = hex.to_string().parse()?;
        Ok(Arc::new(Self { inner }))
    }

    /// Return the bytes of the transaction identifier.
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.as_byte_array().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use crate::Txid;

    #[test]
    fn txid() {
        let expected_txid = "0000000000000000000000000000000000000000000000000000000000000001";

        let txid = Txid::new(&expected_txid.parse().unwrap()).unwrap();
        assert_eq!(txid.to_string(), expected_txid);
        assert_eq!(txid.bytes()[0], 1);
    }
}
