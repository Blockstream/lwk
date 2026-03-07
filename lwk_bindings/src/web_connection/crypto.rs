use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};

use crate::LwkError;

const WALLET_ABI_RELAY_CHANNEL_KEY_BYTES: usize = 32;
const WALLET_ABI_RELAY_NONCE_BYTES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WalletAbiRelayDirection {
    WebToPhone,
    PhoneToWeb,
}

#[derive(Debug, Clone)]
pub(crate) struct WalletAbiRelayEncryptedPayload {
    pub(crate) nonce_b64: String,
    pub(crate) ciphertext_b64: String,
}

pub(crate) fn encrypt_relay_payload(
    channel_key_b64: &str,
    pairing_id: &str,
    direction: WalletAbiRelayDirection,
    msg_id: &str,
    plaintext: &[u8],
) -> Result<WalletAbiRelayEncryptedPayload, LwkError> {
    let channel_key = decode_channel_key_b64(channel_key_b64.trim())?;
    ensure_non_empty_field("pairing_id", pairing_id.trim())?;
    ensure_non_empty_field("msg_id", msg_id.trim())?;

    let mut nonce = [0u8; WALLET_ABI_RELAY_NONCE_BYTES];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay nonce generation failed".to_string(),
        })?;

    let derived_key = derive_directional_key(&channel_key, pairing_id.trim(), direction)?;
    let aad = build_relay_aad(pairing_id.trim(), direction, msg_id.trim());

    let cipher =
        XChaCha20Poly1305::new_from_slice(&derived_key).map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay cipher initialization failed".to_string(),
        })?;
    let nonce = XNonce::from(nonce);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay encryption failed".to_string(),
        })?;

    Ok(WalletAbiRelayEncryptedPayload {
        nonce_b64: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext_b64: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

pub(crate) fn decrypt_relay_payload(
    channel_key_b64: &str,
    pairing_id: &str,
    direction: WalletAbiRelayDirection,
    msg_id: &str,
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> Result<Vec<u8>, LwkError> {
    let channel_key = decode_channel_key_b64(channel_key_b64.trim())?;
    ensure_non_empty_field("pairing_id", pairing_id.trim())?;
    ensure_non_empty_field("msg_id", msg_id.trim())?;

    let nonce = decode_nonce_b64(nonce_b64.trim())?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(ciphertext_b64.trim().as_bytes())
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay ciphertext decode failed: {error}"),
        })?;

    let derived_key = derive_directional_key(&channel_key, pairing_id.trim(), direction)?;
    let aad = build_relay_aad(pairing_id.trim(), direction, msg_id.trim());

    let cipher =
        XChaCha20Poly1305::new_from_slice(&derived_key).map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay cipher initialization failed".to_string(),
        })?;
    let nonce = XNonce::from(nonce);
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay decryption failed".to_string(),
        })
}

pub(crate) fn decode_channel_key_b64(
    encoded: &str,
) -> Result<[u8; WALLET_ABI_RELAY_CHANNEL_KEY_BYTES], LwkError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay channel key decode failed: {error}"),
        })?;

    if decoded.len() != WALLET_ABI_RELAY_CHANNEL_KEY_BYTES {
        return Err(LwkError::Generic {
            msg: format!(
                "wallet-abi relay channel key must be {WALLET_ABI_RELAY_CHANNEL_KEY_BYTES} bytes"
            ),
        });
    }

    let mut key = [0u8; WALLET_ABI_RELAY_CHANNEL_KEY_BYTES];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn decode_nonce_b64(encoded: &str) -> Result<[u8; WALLET_ABI_RELAY_NONCE_BYTES], LwkError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay nonce decode failed: {error}"),
        })?;

    if decoded.len() != WALLET_ABI_RELAY_NONCE_BYTES {
        return Err(LwkError::Generic {
            msg: format!("wallet-abi relay nonce must be {WALLET_ABI_RELAY_NONCE_BYTES} bytes"),
        });
    }

    let mut nonce = [0u8; WALLET_ABI_RELAY_NONCE_BYTES];
    nonce.copy_from_slice(&decoded);
    Ok(nonce)
}

fn derive_directional_key(
    channel_key: &[u8; WALLET_ABI_RELAY_CHANNEL_KEY_BYTES],
    pairing_id: &str,
    direction: WalletAbiRelayDirection,
) -> Result<[u8; 32], LwkError> {
    ensure_non_empty_field("pairing_id", pairing_id)?;

    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, pairing_id.as_bytes());
    let prk = salt.extract(channel_key);
    let info = [
        b"wallet_abi_relay_v1".as_slice(),
        direction.hkdf_info_label(),
    ];
    let okm = prk
        .expand(&info, HkdfLen(32))
        .map_err(|_| LwkError::Generic {
            msg: "wallet-abi relay key derivation failed".to_string(),
        })?;

    let mut derived = [0u8; 32];
    okm.fill(&mut derived).map_err(|_| LwkError::Generic {
        msg: "wallet-abi relay key derivation failed".to_string(),
    })?;

    Ok(derived)
}

fn build_relay_aad(pairing_id: &str, direction: WalletAbiRelayDirection, msg_id: &str) -> Vec<u8> {
    format!("{pairing_id}|{}|{msg_id}", direction.aad_name()).into_bytes()
}

fn ensure_non_empty_field(field: &str, value: &str) -> Result<(), LwkError> {
    if value.is_empty() {
        return Err(LwkError::Generic {
            msg: format!("{field} must not be empty"),
        });
    }
    Ok(())
}

impl WalletAbiRelayDirection {
    fn hkdf_info_label(self) -> &'static [u8] {
        match self {
            WalletAbiRelayDirection::WebToPhone => b"web_to_phone",
            WalletAbiRelayDirection::PhoneToWeb => b"phone_to_web",
        }
    }

    fn aad_name(self) -> &'static str {
        match self {
            WalletAbiRelayDirection::WebToPhone => "web_to_phone",
            WalletAbiRelayDirection::PhoneToWeb => "phone_to_web",
        }
    }
}

#[derive(Clone, Copy)]
struct HkdfLen(usize);

impl hkdf::KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ring::rand::{SecureRandom, SystemRandom};

    use super::{decrypt_relay_payload, encrypt_relay_payload, WalletAbiRelayDirection};
    use crate::LwkError;

    #[test]
    fn wallet_abi_relay_encrypt_decrypt_roundtrip() {
        let channel_key_b64 = sample_channel_key_b64();
        let payload = encrypt_relay_payload(
            &channel_key_b64,
            "pairing-1",
            WalletAbiRelayDirection::WebToPhone,
            "msg-1",
            b"{\"request_id\":\"req-1\"}",
        )
        .expect("encrypt");

        let plaintext = decrypt_relay_payload(
            &channel_key_b64,
            "pairing-1",
            WalletAbiRelayDirection::WebToPhone,
            "msg-1",
            &payload.nonce_b64,
            &payload.ciphertext_b64,
        )
        .expect("decrypt");

        assert_eq!(plaintext, b"{\"request_id\":\"req-1\"}".to_vec());
    }

    #[test]
    fn wallet_abi_relay_decrypt_rejects_mismatched_aad() {
        let channel_key_b64 = sample_channel_key_b64();
        let payload = encrypt_relay_payload(
            &channel_key_b64,
            "pairing-1",
            WalletAbiRelayDirection::WebToPhone,
            "msg-1",
            b"{\"request_id\":\"req-1\"}",
        )
        .expect("encrypt");

        let err = decrypt_relay_payload(
            &channel_key_b64,
            "pairing-1",
            WalletAbiRelayDirection::WebToPhone,
            "msg-2",
            &payload.nonce_b64,
            &payload.ciphertext_b64,
        )
        .expect_err("aad mismatch must fail");

        match err {
            LwkError::Generic { msg } => assert!(msg.contains("decryption failed")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    fn sample_channel_key_b64() -> String {
        let mut key = [0u8; 32];
        SystemRandom::new()
            .fill(&mut key)
            .expect("random channel key");
        URL_SAFE_NO_PAD.encode(key)
    }
}
