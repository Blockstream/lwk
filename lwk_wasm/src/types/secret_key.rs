use crate::{Error, Pset};

use lwk_wollet::elements::bitcoin::secp256k1;

use wasm_bindgen::prelude::*;

/// A secret key
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct SecretKey {
    inner: secp256k1::SecretKey,
}

impl From<secp256k1::SecretKey> for SecretKey {
    fn from(inner: secp256k1::SecretKey) -> Self {
        SecretKey { inner }
    }
}

impl From<SecretKey> for secp256k1::SecretKey {
    fn from(value: SecretKey) -> Self {
        value.inner
    }
}

impl From<&SecretKey> for secp256k1::SecretKey {
    fn from(value: &SecretKey) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl SecretKey {
    /// Creates a `SecretKey` from a 32-byte array
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<SecretKey, Error> {
        let inner = secp256k1::SecretKey::from_slice(bytes)?;
        Ok(SecretKey { inner })
    }

    /// Creates a `SecretKey` from a WIF (Wallet Import Format) string
    #[wasm_bindgen(js_name = fromWif)]
    pub fn from_wif(wif: &str) -> Result<SecretKey, Error> {
        let inner = lwk_wollet::elements::bitcoin::PrivateKey::from_wif(wif)?.inner;
        Ok(SecretKey { inner })
    }

    /// Returns the bytes of the secret key (32 bytes)
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.secret_bytes().to_vec()
    }

    /// Sign the given `pset`
    pub fn sign(&self, pset: Pset) -> Result<Pset, Error> {
        let mut pset_inner = pset.into();
        lwk_signer::sign_with_seckey(self.inner, &mut pset_inner)?;
        Ok(pset_inner.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_secret_key() {
        let bytes = [0xcd; 32];
        let sk = SecretKey::new(&bytes).unwrap();
        assert_eq!(sk.bytes(), bytes);

        let wif_test = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
        let wif_main = "L2wTu6hQrnDMiFNWA5na6jB12ErGQqtXwqpSL7aWquJaZG8Ai3ch";
        let key_test = SecretKey::from_wif(wif_test).unwrap();
        let key_main = SecretKey::from_wif(wif_main).unwrap();
        assert_eq!(key_test.bytes(), key_main.bytes());

        assert!(SecretKey::new(&[0; 31]).is_err());
        assert!(SecretKey::new(&[0; 33]).is_err());
        assert!(SecretKey::new(&[0; 32]).is_err());
    }
}
