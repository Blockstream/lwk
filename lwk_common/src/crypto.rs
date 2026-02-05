//! Shared encryption helpers.

#[allow(deprecated)]
use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::AeadMutInPlace;
use aes_gcm_siv::{Aes256GcmSiv, KeyInit};
use elements::bitcoin::hashes::{sha256t_hash_newtype, Hash};
use rand::{thread_rng, Rng};

/// Length in bytes of an AES-GCM-SIV nonce.
pub const NONCE_LEN: usize = 12;

sha256t_hash_newtype! {
    /// Tag for deterministic nonce derivation.
    pub struct DeterministicNonceTag = hash_str("LWK-Deterministic-Nonce/1.0");

    /// Tagged hash for deterministic nonce derivation.
    #[hash_newtype(forward)]
    pub struct DeterministicNonceHash(_);
}

/// Errors returned by the crypto helpers.
#[derive(Debug)]
pub enum CryptoError {
    /// Missing nonce prefix in encrypted payloads.
    MissingNonce,
    /// AEAD encryption/decryption error.
    Aead(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::MissingNonce => write!(f, "Encrypted data too short - missing nonce"),
            CryptoError::Aead(err) => write!(f, "Aead error: {err}"),
        }
    }
}

/// Create a cipher from 32 key bytes.
#[allow(deprecated)]
pub fn cipher_from_key_bytes(key_bytes: [u8; 32]) -> Aes256GcmSiv {
    let key = GenericArray::from_slice(&key_bytes);
    Aes256GcmSiv::new(key)
}

/// Encrypt a payload using the provided nonce, returning `nonce || ciphertext`.
#[allow(deprecated)]
fn encrypt_with_nonce(
    cipher: &mut Aes256GcmSiv,
    plaintext: &[u8],
    nonce_bytes: [u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let mut buffer = plaintext.to_vec();
    cipher
        .encrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|err| CryptoError::Aead(err.to_string()))?;

    let mut result = Vec::with_capacity(nonce_bytes.len() + buffer.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&buffer);
    Ok(result)
}

/// Encrypt a payload with a random nonce.
///
/// The nonce is prepended to the ciphertext for later decryption.
///
/// NOTE: `allow(deprecated)` cannot be removed until aes-gcm-siv 0.12 is released
#[allow(deprecated)]
pub fn encrypt_with_random_nonce(
    cipher: &mut Aes256GcmSiv,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    thread_rng().fill(&mut nonce_bytes);
    encrypt_with_nonce(cipher, plaintext, nonce_bytes)
}

/// Decrypt a payload that was encrypted with [`encrypt_with_random_nonce`] or [`encrypt_with_deterministic_nonce`].
#[allow(deprecated)]
pub fn decrypt_with_nonce_prefix(
    cipher: &mut Aes256GcmSiv,
    data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_LEN {
        return Err(CryptoError::MissingNonce);
    }

    let nonce_bytes: [u8; NONCE_LEN] = data[..NONCE_LEN]
        .try_into()
        .expect("nonce slice length validated");
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let mut buffer = data[NONCE_LEN..].to_vec();
    cipher
        .decrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|err| CryptoError::Aead(err.to_string()))?;

    Ok(buffer)
}

/// Encrypt a payload using a deterministic nonce derived from the plaintext tagged hash.
///
/// The nonce is prepended to the ciphertext for later decryption.
///
/// NOTE: for normal usage we could have avoided to prefix the nonce to the ciphertext, equality is
/// guaranteed anyway but we keep the prefix to allow decryption in case we need to do db
/// migrations or reconstruction.
#[allow(deprecated)]
pub fn encrypt_with_deterministic_nonce(
    cipher: &mut Aes256GcmSiv,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let hash = DeterministicNonceHash::hash(plaintext);
    let nonce_bytes: [u8; NONCE_LEN] = hash.as_byte_array()[..NONCE_LEN]
        .try_into()
        .expect("nonce slice length validated");
    encrypt_with_nonce(cipher, plaintext, nonce_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cipher() -> Aes256GcmSiv {
        cipher_from_key_bytes([7u8; 32])
    }

    #[test]
    fn random_nonce_roundtrip() {
        let mut cipher = test_cipher();
        let plaintext = b"example plaintext";
        let encrypted = encrypt_with_random_nonce(&mut cipher, plaintext).unwrap();
        assert!(encrypted.len() > NONCE_LEN);

        let mut cipher = test_cipher();
        let decrypted = decrypt_with_nonce_prefix(&mut cipher, &encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn deterministic_nonce_is_stable() {
        let plaintext = b"deterministic payload";
        let mut cipher = test_cipher();
        let encrypted1 = encrypt_with_deterministic_nonce(&mut cipher, plaintext).unwrap();
        assert!(encrypted1.len() > NONCE_LEN);

        let mut cipher = test_cipher();
        let decrypted1 = decrypt_with_nonce_prefix(&mut cipher, &encrypted1).unwrap();
        assert_eq!(&plaintext[..], &decrypted1[..]);

        let mut cipher = test_cipher();
        let encrypted2 = encrypt_with_deterministic_nonce(&mut cipher, plaintext).unwrap();
        assert_eq!(encrypted1, encrypted2);

        let mut cipher = test_cipher();
        let decrypted2 = decrypt_with_nonce_prefix(&mut cipher, &encrypted2).unwrap();
        assert_eq!(&plaintext[..], &decrypted2[..]);
    }
}
