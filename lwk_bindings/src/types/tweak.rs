use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::secp256k1_zkp;

use crate::LwkError;

/// Represents a blinding factor/Tweak on secp256k1 curve
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
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
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner = secp256k1_zkp::Tweak::from_str(s)?;
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
}

#[cfg(test)]
mod tests {
    use super::Tweak;

    #[test]
    fn test_tweak_from_bytes_and_roundtrips() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

        let from_hex = Tweak::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);

        let bytes = from_hex.to_bytes();
        let from_bytes = Tweak::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(from_bytes.to_string(), hex);
        assert_eq!(
            Tweak::from_string(&from_bytes.to_string()).unwrap(),
            from_bytes
        );

        assert!(Tweak::from_bytes(&[0u8; 31]).is_err());
        assert!(Tweak::from_bytes(&[0u8; 33]).is_err());

        let tweak = Tweak::zero();
        assert_eq!(tweak.to_bytes(), vec![0u8; 32]);
        assert_eq!(
            tweak.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
