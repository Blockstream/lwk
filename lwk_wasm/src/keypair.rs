use crate::{Error, PublicKey, SecretKey, XOnlyPublicKey};

use lwk_wollet::elements::bitcoin::secp256k1::{self, Message, Secp256k1};
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;
use lwk_wollet::secp256k1::rand::thread_rng;
use lwk_wollet::EC;

use wasm_bindgen::prelude::*;

/// A secp256k1 keypair
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Keypair {
    inner: secp256k1::Keypair,
}

impl From<secp256k1::Keypair> for Keypair {
    fn from(inner: secp256k1::Keypair) -> Self {
        Keypair { inner }
    }
}

impl From<Keypair> for secp256k1::Keypair {
    fn from(value: Keypair) -> Self {
        value.inner
    }
}

impl From<&Keypair> for secp256k1::Keypair {
    fn from(value: &Keypair) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl Keypair {
    /// Creates a `Keypair` from a 32-byte secret key
    #[wasm_bindgen(constructor)]
    pub fn new(secret_bytes: &[u8]) -> Result<Keypair, Error> {
        let secp = Secp256k1::new();
        let inner = secp256k1::Keypair::from_seckey_slice(&secp, secret_bytes)?;
        Ok(Keypair { inner })
    }

    /// Creates a `Keypair` from a `SecretKey`
    #[wasm_bindgen(js_name = fromSecretKey)]
    pub fn from_secret_key(sk: &SecretKey) -> Keypair {
        let secp = Secp256k1::new();
        let secret: secp256k1::SecretKey = sk.into();
        let inner = secp256k1::Keypair::from_secret_key(&secp, &secret);
        Keypair { inner }
    }

    /// Generates a new random keypair
    pub fn generate() -> Keypair {
        let secp = Secp256k1::new();
        let inner = secp256k1::Keypair::new(&secp, &mut thread_rng());
        Keypair { inner }
    }

    /// Returns the secret key bytes (32 bytes)
    #[wasm_bindgen(js_name = secretBytes)]
    pub fn secret_bytes(&self) -> Vec<u8> {
        self.inner.secret_bytes().to_vec()
    }

    /// Returns the `SecretKey`
    #[wasm_bindgen(js_name = secretKey)]
    pub fn secret_key(&self) -> SecretKey {
        self.inner.secret_key().into()
    }

    /// Returns the `PublicKey`
    #[wasm_bindgen(js_name = publicKey)]
    pub fn public_key(&self) -> PublicKey {
        let pk = lwk_wollet::elements::bitcoin::PublicKey::new(self.inner.public_key());
        pk.into()
    }

    /// Returns the x-only public key
    #[wasm_bindgen(js_name = xOnlyPublicKey)]
    pub fn x_only_public_key(&self) -> XOnlyPublicKey {
        let (xonly, _parity) = self.inner.x_only_public_key();
        xonly.into()
    }

    /// Signs a 32-byte message hash using Schnorr signature
    ///
    /// Takes the message as a hex string (64 hex chars = 32 bytes)
    /// Returns the signature as a hex string (128 hex chars = 64 bytes)
    #[wasm_bindgen(js_name = signSchnorr)]
    pub fn sign_schnorr(&self, msg_hex: &str) -> Result<String, Error> {
        let msg_bytes = Vec::<u8>::from_hex(msg_hex)?;
        let msg_array: [u8; 32] = msg_bytes.as_slice().try_into().map_err(|_| {
            Error::Generic(format!(
                "Message must be exactly 32 bytes, got {}",
                msg_bytes.len()
            ))
        })?;
        let message = Message::from_digest(msg_array);
        let sig = EC.sign_schnorr(&message, &self.inner);
        Ok(sig.serialize().to_hex())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::SecretKey;

    use lwk_wollet::secp256k1::schnorr;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_keypair_from_secret_bytes() {
        let bytes = [1u8; 32];
        let kp = Keypair::new(&bytes).unwrap();
        assert_eq!(kp.secret_bytes(), bytes);
    }

    #[wasm_bindgen_test]
    fn test_keypair_from_secret_key() {
        let sk = SecretKey::new(&[1u8; 32]).unwrap();
        let kp = Keypair::from_secret_key(&sk);
        assert_eq!(kp.secret_bytes(), sk.bytes());
    }

    #[wasm_bindgen_test]
    fn test_keypair_generate() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        assert_ne!(kp1.secret_bytes(), kp2.secret_bytes());
    }

    #[wasm_bindgen_test]
    fn test_keypair_public_key() {
        let kp = Keypair::new(&[1u8; 32]).unwrap();
        let pk = kp.public_key();
        assert_eq!(pk.to_bytes().len(), 33);
    }

    #[wasm_bindgen_test]
    fn test_keypair_x_only_public_key() {
        let kp = Keypair::new(&[1u8; 32]).unwrap();
        let xonly = kp.x_only_public_key();
        assert_eq!(xonly.to_hex().len(), 64);
    }

    #[wasm_bindgen_test]
    fn test_keypair_secret_key() {
        let bytes = [1u8; 32];
        let kp = Keypair::new(&bytes).unwrap();
        let sk = kp.secret_key();
        assert_eq!(sk.bytes(), bytes);
    }

    #[wasm_bindgen_test]
    fn test_keypair_sign_schnorr() {
        let kp = Keypair::new(&[1u8; 32]).unwrap();
        let msg_hex = "0202020202020202020202020202020202020202020202020202020202020202";

        let sig_hex = kp.sign_schnorr(msg_hex).unwrap();
        assert_eq!(sig_hex.len(), 128); // 64 bytes = 128 hex chars

        // Verify the signature
        let sig = schnorr::Signature::from_slice(&Vec::<u8>::from_hex(&sig_hex).unwrap()).unwrap();
        let msg_bytes = Vec::<u8>::from_hex(msg_hex).unwrap();
        let message = Message::from_digest(msg_bytes.try_into().unwrap());

        let (xonly, _) = kp.inner.x_only_public_key();
        assert!(EC.verify_schnorr(&sig, &message, &xonly).is_ok());
    }

    #[wasm_bindgen_test]
    fn test_keypair_invalid_secret() {
        // All zeros is invalid
        assert!(Keypair::new(&[0; 32]).is_err());
        // Wrong length
        assert!(Keypair::new(&[1; 31]).is_err());
        assert!(Keypair::new(&[1; 33]).is_err());
    }
}
