use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::hex::ToHex;
use elements::secp256k1_zkp;

use crate::LwkError;

/// Represents a blinding factor/Tweak on secp256k1 curve
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct Tweak {
    inner: secp256k1_zkp::Tweak,
}

impl From<secp256k1_zkp::Tweak> for Tweak {
    fn from(inner: secp256k1_zkp::Tweak) -> Self {
        Tweak { inner }
    }
}

impl From<Tweak> for secp256k1_zkp::Tweak {
    fn from(value: Tweak) -> Self {
        value.inner
    }
}

impl From<&Tweak> for secp256k1_zkp::Tweak {
    fn from(value: &Tweak) -> Self {
        value.inner
    }
}

impl AsRef<secp256k1_zkp::Tweak> for Tweak {
    fn as_ref(&self) -> &secp256k1_zkp::Tweak {
        &self.inner
    }
}

impl FromStr for Tweak {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = secp256k1_zkp::Tweak::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for Tweak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Tweak {
    /// Create a Tweak from a 32-byte slice.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = secp256k1_zkp::Tweak::from_slice(bytes)?;
        Ok(Arc::new(Tweak { inner }))
    }

    /// Create a Tweak from a hex string.
    #[uniffi::constructor]
    pub fn from_hex(hex: &str) -> Result<Arc<Self>, LwkError> {
        let inner = secp256k1_zkp::Tweak::from_str(hex)?;
        Ok(Arc::new(Tweak { inner }))
    }

    /// Create the zero tweak.
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(Tweak {
            inner: secp256k1_zkp::ZERO_TWEAK,
        })
    }

    /// Return the bytes of the tweak (32 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.as_ref().to_vec()
    }

    /// Return the hex representation of the tweak.
    pub fn to_hex(&self) -> String {
        self.inner.as_ref().to_hex()
    }
}

#[cfg(test)]
mod tests {
    use super::Tweak;

    #[test]
    fn test_tweak_zero() {
        let tweak = Tweak::zero();
        assert_eq!(tweak.to_bytes(), vec![0u8; 32]);
    }

    #[test]
    fn test_tweak_from_bytes() {
        let bytes = [1u8; 32];
        let tweak = Tweak::from_bytes(&bytes).unwrap();
        assert_eq!(tweak.to_bytes(), bytes);
    }

    #[test]
    fn test_tweak_from_hex() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let tweak = Tweak::from_hex(hex).unwrap();
        assert_eq!(tweak.to_hex(), hex);
    }

    #[test]
    fn test_tweak_roundtrip() {
        let bytes = [2u8; 32];
        let tweak = Tweak::from_bytes(&bytes).unwrap();
        let tweak2 = Tweak::from_hex(&tweak.to_hex()).unwrap();
        assert_eq!(*tweak, *tweak2);
    }

    #[test]
    fn test_tweak_display() {
        let tweak = Tweak::zero();
        assert_eq!(
            tweak.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
