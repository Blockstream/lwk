use crate::Error;

use std::str::FromStr;

use lwk_simplicity::simplicityhl::simplicity;

use wasm_bindgen::prelude::*;

/// A Simplicity Commitment Merkle Root (CMR).
///
/// See [`simplicity::Cmr`] for more details.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct Cmr {
    inner: simplicity::Cmr,
}

impl From<simplicity::Cmr> for Cmr {
    fn from(inner: simplicity::Cmr) -> Self {
        Self { inner }
    }
}

impl Cmr {
    pub(crate) fn inner(&self) -> simplicity::Cmr {
        self.inner
    }
}

#[wasm_bindgen]
impl Cmr {
    /// Create a CMR from hex (64 hex characters = 32 bytes).
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<Cmr, Error> {
        Self::from_hex(hex)
    }

    /// Create a CMR from bytes (32 bytes).
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Cmr, Error> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::Generic(format!("expected 32 bytes, got {}", bytes.len())))?;
        Ok(Self {
            inner: simplicity::Cmr::from_byte_array(arr),
        })
    }

    /// Create a CMR from hex (64 hex characters = 32 bytes).
    #[wasm_bindgen(js_name = fromHex)]
    pub fn from_hex(hex: &str) -> Result<Cmr, Error> {
        Ok(simplicity::Cmr::from_str(&hex.to_string())?.into())
    }

    /// Return the hex-encoded CMR (64 hex characters).
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.to_string()
    }

    /// Return the raw CMR bytes (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use crate::simplicity::{SimplicityArguments, SimplicityProgram, SimplicityTypedValue};

    use lwk_wollet::hashes::hex::FromHex;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    const TEST_CMR: &str = "b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d02";
    const INVALID_CMR_HEX: &str =
        "zz85a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d02";
    const TEST_PUBLIC_KEY: &str =
        "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083";
    const P2PK_SOURCE: &str = include_str!("../../../lwk_simplicity/data/p2pk.simf");

    #[wasm_bindgen_test]
    fn test_cmr_hex_validation_and_roundtrips() {
        let expected_bytes = Vec::<u8>::from_hex(TEST_CMR).unwrap();

        let cmr = Cmr::from_hex(TEST_CMR).unwrap();
        assert_eq!(cmr.to_hex(), TEST_CMR);
        assert_eq!(cmr.to_bytes(), expected_bytes.clone());

        let from_bytes = Cmr::from_bytes(&expected_bytes).unwrap();
        assert_eq!(from_bytes.to_hex(), TEST_CMR);
        assert_eq!(from_bytes.to_bytes(), expected_bytes.clone());
        assert_eq!(Cmr::new(TEST_CMR).unwrap().to_hex(), TEST_CMR);

        assert!(Cmr::from_bytes(&[0u8; 31]).is_err());
        assert!(Cmr::from_bytes(&[0u8; 33]).is_err());
        assert!(Cmr::from_bytes(&[]).is_err());
        assert!(Cmr::from_hex("0011").is_err());
        assert!(Cmr::from_hex(INVALID_CMR_HEX).is_err());

        let args = SimplicityArguments::new().add_value(
            "PUBLIC_KEY",
            &SimplicityTypedValue::from_u256_hex(TEST_PUBLIC_KEY).unwrap(),
        );
        let program = SimplicityProgram::new(P2PK_SOURCE, &args).unwrap();
        let cmr = program.cmr();
        assert_eq!(cmr.to_hex(), TEST_CMR);
        assert_eq!(cmr.to_bytes(), expected_bytes);
    }
}
