use std::{fmt::Display, str::FromStr};

use elements::{hashes::hex::FromHex, hex::ToHex};

use crate::UniffiCustomTypeConverter;

/// A valid hex string.
///
/// Even number of characters and only numerical and from 'a' to 'e'
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Hex {
    inner: Vec<u8>,
}

impl From<Vec<u8>> for Hex {
    fn from(inner: Vec<u8>) -> Self {
        Hex { inner }
    }
}

impl From<&[u8]> for Hex {
    fn from(slice: &[u8]) -> Self {
        Hex {
            inner: slice.to_vec(),
        }
    }
}

impl FromStr for Hex {
    type Err = elements::hashes::hex::HexToBytesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Vec::<u8>::from_hex(s)?.into())
    }
}

impl AsRef<[u8]> for Hex {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.to_hex()) // TODO: do without allocating
    }
}

uniffi::custom_type!(Hex, String);
impl UniffiCustomTypeConverter for Hex {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        let inner = Vec::<u8>::from_hex(&val)?;
        Ok(Hex { inner })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.inner.to_hex()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Hex;
    use crate::UniffiCustomTypeConverter;

    #[test]
    fn hex() {
        let hex: Hex = Hex::from_str("aa").unwrap();
        assert_eq!(
            <Hex as UniffiCustomTypeConverter>::into_custom(
                UniffiCustomTypeConverter::from_custom(hex.clone())
            )
            .unwrap(),
            hex
        );
    }
}
