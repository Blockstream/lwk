use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use url::Url;

use super::crypto::decode_channel_key_b64;
use crate::LwkError;

const WALLET_ABI_RELAY_PAIRING_PARAM: &str = "wa_relay_v1";
const WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayAlgorithm {
    Xchacha20poly1305HkdfSha256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalletAbiRelayPairing {
    pub(crate) v: u64,
    pub(crate) pairing_id: String,
    pub(crate) relay_ws_url: String,
    pub(crate) expires_at_ms: u64,
    pub(crate) phone_token: String,
    pub(crate) channel_key_b64: String,
    pub(crate) alg: WalletAbiRelayAlgorithm,
}

/// Normalize relay pairing input into canonical relay-pairing JSON.
///
/// Accepted forms:
/// - raw pairing JSON
/// - full URL/fragment containing `#wa_relay_v1=...`
/// - `wa_relay_v1=...`
/// - raw `wa_relay_v1` payload
#[uniffi::export]
pub fn web_connection_extract_relay_pairing_json(input: String) -> Result<String, LwkError> {
    let pairing = parse_relay_pairing_input(&input)?;
    serde_json::to_string(&pairing).map_err(|error| LwkError::Generic {
        msg: format!("wallet-abi relay pairing serialization failed: {error}"),
    })
}

pub(crate) fn parse_relay_pairing_input(input: &str) -> Result<WalletAbiRelayPairing, LwkError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(LwkError::Generic {
            msg: "wallet-abi relay input must not be empty".to_string(),
        });
    }

    if let Ok(pairing) = serde_json::from_str::<WalletAbiRelayPairing>(trimmed) {
        validate_relay_pairing_metadata(&pairing).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay pairing validation failed: {error}"),
        })?;
        return Ok(pairing);
    }

    let encoded_payload = extract_fragment_param(trimmed, WALLET_ABI_RELAY_PAIRING_PARAM)
        .or_else(|| {
            trimmed
                .strip_prefix(&format!("{WALLET_ABI_RELAY_PAIRING_PARAM}="))
                .map(std::string::ToString::to_string)
        })
        .unwrap_or_else(|| trimmed.to_string());

    let decoded =
        decode_transport_payload(&encoded_payload).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay payload decode failed: {error}"),
        })?;

    let pairing: WalletAbiRelayPairing =
        serde_json::from_slice(&decoded).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay pairing parsing failed: {error}"),
        })?;

    validate_relay_pairing_metadata(&pairing).map_err(|error| LwkError::Generic {
        msg: format!("wallet-abi relay pairing validation failed: {error}"),
    })?;

    Ok(pairing)
}

fn extract_fragment_param(uri_or_fragment: &str, key: &str) -> Option<String> {
    let fragment = uri_or_fragment
        .split_once('#')
        .map_or(uri_or_fragment, |(_, value)| value)
        .trim_start_matches('#')
        .trim();

    if fragment.is_empty() {
        return None;
    }

    fragment.split('&').find_map(|pair| {
        let (candidate_key, candidate_value) = pair.split_once('=')?;
        if candidate_key == key {
            Some(candidate_value.to_string())
        } else {
            None
        }
    })
}

fn decode_transport_payload(encoded: &str) -> Result<Vec<u8>, String> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|error| format!("base64url decode error: {error}"))?;

    let payload = match zstd::stream::decode_all(decoded.as_slice()) {
        Ok(decompressed) => decompressed,
        Err(_) => decoded,
    };

    if payload.len() > WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES {
        return Err(format!(
            "transport payload exceeds {WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES} bytes"
        ));
    }

    Ok(payload)
}

fn validate_relay_pairing_metadata(pairing: &WalletAbiRelayPairing) -> Result<(), String> {
    if pairing.v != 1 {
        return Err("pairing.v must be 1".to_string());
    }

    ensure_non_empty_field("pairing_id", pairing.pairing_id.trim())?;
    ensure_non_empty_field("phone_token", pairing.phone_token.trim())?;

    validate_ws_url(&pairing.relay_ws_url)?;

    if pairing.expires_at_ms == 0 {
        return Err("expires_at_ms must be greater than zero".to_string());
    }

    decode_channel_key_b64(pairing.channel_key_b64.trim()).map_err(|error| match error {
        LwkError::Generic { msg } => msg,
        other => format!("{other}"),
    })?;

    if pairing.alg != WalletAbiRelayAlgorithm::Xchacha20poly1305HkdfSha256 {
        return Err("unsupported relay algorithm".to_string());
    }

    Ok(())
}

fn validate_ws_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url).map_err(|_| "invalid relay_ws_url".to_string())?;

    if parsed.scheme() != "ws" && parsed.scheme() != "wss" {
        return Err("relay_ws_url must use ws:// or wss://".to_string());
    }

    if parsed.host_str().is_none() {
        return Err("relay_ws_url must include a host".to_string());
    }

    Ok(())
}

fn ensure_non_empty_field(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ring::rand::{SecureRandom, SystemRandom};

    use super::{
        web_connection_extract_relay_pairing_json, WalletAbiRelayAlgorithm, WalletAbiRelayPairing,
        WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES,
    };
    use crate::LwkError;

    #[test]
    fn wallet_abi_extract_relay_pairing_json_accepts_wa_relay_v1_fragment() {
        let pairing = sample_relay_pairing();
        let serialized = serde_json::to_vec(&pairing).expect("pairing serialized");
        let compressed = zstd::stream::encode_all(serialized.as_slice(), 0).expect("compressed");
        let encoded = URL_SAFE_NO_PAD.encode(compressed);
        let input = format!("https://wallet.example/request#wa_relay_v1={encoded}");

        let extracted = web_connection_extract_relay_pairing_json(input).expect("relay pairing");
        let parsed: WalletAbiRelayPairing =
            serde_json::from_str(&extracted).expect("parsed relay pairing");
        assert_eq!(parsed.pairing_id, pairing.pairing_id);
        assert_eq!(parsed.relay_ws_url, pairing.relay_ws_url);
    }

    #[test]
    fn wallet_abi_extract_relay_pairing_json_rejects_malformed_payload() {
        let err = web_connection_extract_relay_pairing_json("wa_relay_v1=%%%%".to_string())
            .expect_err("malformed relay payload must fail");
        match err {
            LwkError::Generic { msg } => assert!(msg.contains("payload decode failed")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_extract_relay_pairing_json_rejects_oversize_payload() {
        let oversized = vec![0x01u8; WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES + 1];
        let encoded = URL_SAFE_NO_PAD.encode(oversized);
        let err = web_connection_extract_relay_pairing_json(format!("wa_relay_v1={encoded}"))
            .expect_err("oversize payload must fail");
        match err {
            LwkError::Generic { msg } => assert!(msg.contains("exceeds")),
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

    fn sample_relay_pairing() -> WalletAbiRelayPairing {
        WalletAbiRelayPairing {
            v: 1,
            pairing_id: "pairing-1".to_string(),
            relay_ws_url: "ws://127.0.0.1:8787/v1/ws".to_string(),
            expires_at_ms: 1_700_000_120_000,
            phone_token: "phone-token".to_string(),
            channel_key_b64: sample_channel_key_b64(),
            alg: WalletAbiRelayAlgorithm::Xchacha20poly1305HkdfSha256,
        }
    }
}
