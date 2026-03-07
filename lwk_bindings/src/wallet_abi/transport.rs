use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use lwk_simplicity::wallet_abi::schema::TxCreateRequest;
use serde::Deserialize;

use crate::LwkError;

const WALLET_ABI_TRANSPORT_REQUEST_PARAM: &str = "wa_v1";
const WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
struct WalletAbiTransportRequestV1 {
    tx_create_request: TxCreateRequest,
}

/// Normalize wallet-abi transport input into canonical `TxCreateRequest` JSON.
///
/// Accepted forms:
/// - raw `TxCreateRequest` JSON
/// - full URL/fragment containing `#wa_v1=...`
/// - `wa_v1=...`
/// - raw `wa_v1` payload
#[uniffi::export]
pub fn wallet_abi_extract_request_json(input: String) -> Result<String, LwkError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(LwkError::Generic {
            msg: "wallet-abi input must not be empty".to_string(),
        });
    }

    if let Ok(request) = serde_json::from_str::<TxCreateRequest>(trimmed) {
        return serde_json::to_string(&request).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi request json serialization failed: {error}"),
        });
    }

    let encoded_payload = extract_fragment_param(trimmed, WALLET_ABI_TRANSPORT_REQUEST_PARAM)
        .or_else(|| {
            trimmed
                .strip_prefix(&format!("{WALLET_ABI_TRANSPORT_REQUEST_PARAM}="))
                .map(std::string::ToString::to_string)
        })
        .unwrap_or_else(|| trimmed.to_string());

    let decoded =
        decode_transport_payload(&encoded_payload).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi transport decode failed: {error}"),
        })?;
    let envelope: WalletAbiTransportRequestV1 =
        serde_json::from_slice(&decoded).map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi transport request parsing failed: {error}"),
        })?;

    serde_json::to_string(&envelope.tx_create_request).map_err(|error| LwkError::Generic {
        msg: format!("wallet-abi request json serialization failed: {error}"),
    })
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

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use lwk_simplicity::wallet_abi::schema::{
        generate_request_id, RuntimeParams, TxCreateRequest, TX_CREATE_ABI_VERSION,
    };
    use serde::Serialize;

    use super::{wallet_abi_extract_request_json, WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES};
    use crate::LwkError;

    #[derive(Debug, Serialize)]
    struct TransportRequestEnvelope<'a> {
        v: u64,
        kind: &'a str,
        request_id: String,
        origin: &'a str,
        created_at_ms: u64,
        expires_at_ms: u64,
        callback: serde_json::Value,
        tx_create_request: &'a TxCreateRequest,
    }

    #[test]
    fn wallet_abi_extract_request_json_accepts_raw_request_json() {
        let request = TxCreateRequest {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: generate_request_id(),
            network: lwk_common::Network::LocaltestLiquid,
            params: RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: Some(0.1),
                lock_time: None,
            },
            broadcast: false,
        };
        let raw = serde_json::to_string(&request).expect("request json");

        let extracted = wallet_abi_extract_request_json(raw).expect("extract json");
        let parsed: TxCreateRequest = serde_json::from_str(&extracted).expect("parsed request");
        assert_eq!(parsed.request_id, request.request_id);
    }

    #[test]
    fn wallet_abi_extract_request_json_accepts_wa_v1_fragment() {
        let request = TxCreateRequest {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: generate_request_id(),
            network: lwk_common::Network::TestnetLiquid,
            params: RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            broadcast: false,
        };
        let envelope = TransportRequestEnvelope {
            v: 1,
            kind: "tx_create",
            request_id: request.request_id.to_string(),
            origin: "https://dapp.example",
            created_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_120_000,
            callback: serde_json::json!({"mode":"qr_roundtrip"}),
            tx_create_request: &request,
        };
        let serialized = serde_json::to_vec(&envelope).expect("serialized envelope");
        let compressed = zstd::stream::encode_all(serialized.as_slice(), 0).expect("compressed");
        let encoded = URL_SAFE_NO_PAD.encode(compressed);
        let input = format!("https://wallet.example/request#wa_v1={encoded}");

        let extracted = wallet_abi_extract_request_json(input).expect("extract json");
        let parsed: TxCreateRequest = serde_json::from_str(&extracted).expect("parsed request");
        assert_eq!(parsed.request_id, request.request_id);
    }

    #[test]
    fn wallet_abi_extract_request_json_rejects_malformed_payload() {
        let err = wallet_abi_extract_request_json("wa_v1=%%%%".to_string())
            .expect_err("malformed payload must fail");
        match err {
            LwkError::Generic { msg } => assert!(msg.contains("transport decode failed")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_extract_request_json_rejects_oversize_payload() {
        let oversized = vec![0x01u8; WALLET_ABI_TRANSPORT_MAX_DECODED_BYTES + 1];
        let encoded = URL_SAFE_NO_PAD.encode(oversized);
        let err = wallet_abi_extract_request_json(format!("wa_v1={encoded}"))
            .expect_err("oversize payload must fail");
        match err {
            LwkError::Generic { msg } => assert!(msg.contains("exceeds")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
