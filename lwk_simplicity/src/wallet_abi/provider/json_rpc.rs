use crate::wallet_abi::schema::{TxCreateRequest, TxCreateResponse};

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};

pub const GET_SIGNER_RECEIVE_ADDRESS_METHOD: &str = "get_signer_receive_address";
pub const GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD: &str = "get_raw_signing_x_only_pubkey";
pub const WALLET_ABI_PROCESS_REQUEST_METHOD: &str = "wallet_abi_process_request";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum JsonRpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

impl JsonRpcVersion {
    fn from_wire(version: &str) -> Option<Self> {
        match version {
            "2.0" => Some(Self::V2),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum JsonRpcMethod {
    GetSignerReceiveAddress,
    GetRawSigningXOnlyPubkey,
    WalletAbiProcessRequest,
    Unknown(String),
}

impl JsonRpcMethod {
    fn from_wire(method: String) -> Self {
        match method.as_str() {
            GET_SIGNER_RECEIVE_ADDRESS_METHOD => Self::GetSignerReceiveAddress,
            GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD => Self::GetRawSigningXOnlyPubkey,
            WALLET_ABI_PROCESS_REQUEST_METHOD => Self::WalletAbiProcessRequest,
            _ => Self::Unknown(method),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::GetSignerReceiveAddress => GET_SIGNER_RECEIVE_ADDRESS_METHOD,
            Self::GetRawSigningXOnlyPubkey => GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
            Self::WalletAbiProcessRequest => WALLET_ABI_PROCESS_REQUEST_METHOD,
            Self::Unknown(method) => method.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JsonRpcErrorCode {
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
}

impl From<JsonRpcErrorCode> for i32 {
    fn from(value: JsonRpcErrorCode) -> Self {
        match value {
            JsonRpcErrorCode::InvalidRequest => -32_600,
            JsonRpcErrorCode::MethodNotFound => -32_601,
            JsonRpcErrorCode::InvalidParams => -32_602,
            JsonRpcErrorCode::InternalError => -32_603,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignerReceiveAddressResult {
    pub signer_receive_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawSigningXOnlyPubkeyResult {
    pub raw_signing_x_only_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub(super) enum JsonRpcResultPayload {
    SignerReceiveAddress(SignerReceiveAddressResult),
    RawSigningXOnlyPubkey(RawSigningXOnlyPubkeyResult),
    TxCreateResponse(TxCreateResponse),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum JsonRpcRequest {
    GetSignerReceiveAddress {
        id: i64,
    },
    GetRawSigningXOnlyPubkey {
        id: i64,
    },
    WalletAbiProcessRequest {
        id: i64,
        tx_create_request: TxCreateRequest,
    },
    Rejected {
        response: JsonRpcResponse,
    },
}

impl<'de> Deserialize<'de> for JsonRpcRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawJsonRpcRequest::deserialize(deserializer)?;
        let id = raw.id;

        if JsonRpcVersion::from_wire(&raw.jsonrpc).is_none() {
            return Ok(Self::Rejected {
                response: JsonRpcResponse::error(
                    id,
                    JsonRpcErrorCode::InvalidRequest,
                    format!("unsupported jsonrpc version '{}'", raw.jsonrpc),
                ),
            });
        }

        match JsonRpcMethod::from_wire(raw.method) {
            JsonRpcMethod::GetSignerReceiveAddress => {
                if !has_no_params(raw.params.as_ref()) {
                    return Ok(Self::Rejected {
                        response: JsonRpcResponse::error(
                            id,
                            JsonRpcErrorCode::InvalidParams,
                            format!(
                                "method '{}' does not accept params",
                                JsonRpcMethod::GetSignerReceiveAddress.as_str()
                            ),
                        ),
                    });
                }
                Ok(Self::GetSignerReceiveAddress { id })
            }
            JsonRpcMethod::GetRawSigningXOnlyPubkey => {
                if !has_no_params(raw.params.as_ref()) {
                    return Ok(Self::Rejected {
                        response: JsonRpcResponse::error(
                            id,
                            JsonRpcErrorCode::InvalidParams,
                            format!(
                                "method '{}' does not accept params",
                                JsonRpcMethod::GetRawSigningXOnlyPubkey.as_str()
                            ),
                        ),
                    });
                }
                Ok(Self::GetRawSigningXOnlyPubkey { id })
            }
            JsonRpcMethod::WalletAbiProcessRequest => {
                let Some(params) = raw.params else {
                    return Ok(Self::Rejected {
                        response: JsonRpcResponse::error(
                            id,
                            JsonRpcErrorCode::InvalidParams,
                            format!(
                                "method '{}' requires object params",
                                JsonRpcMethod::WalletAbiProcessRequest.as_str()
                            ),
                        ),
                    });
                };

                let tx_create_request = match serde_json::from_value(params) {
                    Ok(request) => request,
                    Err(error) => {
                        return Ok(Self::Rejected {
                            response: JsonRpcResponse::error(
                                id,
                                JsonRpcErrorCode::InvalidParams,
                                format!(
                                    "invalid '{}' params: {error}",
                                    JsonRpcMethod::WalletAbiProcessRequest.as_str()
                                ),
                            ),
                        });
                    }
                };

                Ok(Self::WalletAbiProcessRequest {
                    id,
                    tx_create_request,
                })
            }
            JsonRpcMethod::Unknown(method) => Ok(Self::Rejected {
                response: JsonRpcResponse::error(
                    id,
                    JsonRpcErrorCode::MethodNotFound,
                    format!("unsupported method '{}'", method),
                ),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub(super) enum JsonRpcResponse {
    Result(JsonRpcResult),
    Error(JsonRpcErrorResponse),
}

impl JsonRpcResponse {
    pub(super) fn result(id: i64, result: JsonRpcResultPayload) -> Self {
        Self::Result(JsonRpcResult {
            id,
            jsonrpc: JsonRpcVersion::V2,
            result,
        })
    }

    pub(super) fn error(id: i64, code: JsonRpcErrorCode, message: String) -> Self {
        Self::Error(JsonRpcErrorResponse {
            id,
            jsonrpc: JsonRpcVersion::V2,
            error: JsonRpcErrorObject {
                code: code.into(),
                message,
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct JsonRpcResult {
    pub(super) id: i64,
    pub(super) jsonrpc: JsonRpcVersion,
    pub(super) result: JsonRpcResultPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct JsonRpcErrorResponse {
    pub(super) id: i64,
    pub(super) jsonrpc: JsonRpcVersion,
    pub(super) error: JsonRpcErrorObject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct JsonRpcErrorObject {
    pub(super) code: i32,
    pub(super) message: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawJsonRpcRequest {
    id: i64,
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

fn has_no_params(params: Option<&serde_json::Value>) -> bool {
    match params {
        None => true,
        Some(serde_json::Value::Null) => true,
        Some(serde_json::Value::Object(map)) if map.is_empty() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        JsonRpcErrorCode, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse,
        JsonRpcResultPayload, JsonRpcVersion, RawSigningXOnlyPubkeyResult,
        SignerReceiveAddressResult, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
        GET_SIGNER_RECEIVE_ADDRESS_METHOD, WALLET_ABI_PROCESS_REQUEST_METHOD,
    };
    use crate::wallet_abi::schema::{
        generate_request_id, RuntimeParams, TxCreateRequest, TxCreateResponse,
        TX_CREATE_ABI_VERSION,
    };

    #[test]
    fn deserialize_known_methods_into_typed_requests() {
        let signer_receive_address_request = serde_json::from_str::<JsonRpcRequest>(
            &json_rpc_request(1, GET_SIGNER_RECEIVE_ADDRESS_METHOD, None),
        )
        .expect("request parse");
        assert!(matches!(
            signer_receive_address_request,
            JsonRpcRequest::GetSignerReceiveAddress { id: 1 }
        ));

        let signing_x_only_pubkey_request = serde_json::from_str::<JsonRpcRequest>(
            &json_rpc_request(2, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, None),
        )
        .expect("request parse");
        assert!(matches!(
            signing_x_only_pubkey_request,
            JsonRpcRequest::GetRawSigningXOnlyPubkey { id: 2 }
        ));

        let inner_request = sample_tx_create_request();
        let process_request = serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(
            3,
            WALLET_ABI_PROCESS_REQUEST_METHOD,
            Some(serde_json::to_value(&inner_request).expect("inner request")),
        ))
        .expect("request parse");

        match process_request {
            JsonRpcRequest::WalletAbiProcessRequest {
                id,
                tx_create_request,
            } => {
                assert_eq!(id, 3);
                assert_eq!(tx_create_request.request_id, inner_request.request_id);
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }

    #[test]
    fn deserialize_unknown_method_into_json_rpc_rejection() {
        let request =
            serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(4, "unsupported", None))
                .expect("request parse");

        assert_eq!(
            rejected_response(request),
            JsonRpcResponse::Error(JsonRpcErrorResponse {
                id: 4,
                jsonrpc: JsonRpcVersion::V2,
                error: super::JsonRpcErrorObject {
                    code: i32::from(JsonRpcErrorCode::MethodNotFound),
                    message: "unsupported method 'unsupported'".to_string(),
                },
            })
        );
    }

    #[test]
    fn deserialize_unsupported_jsonrpc_version_into_invalid_request() {
        let request = serde_json::from_str::<JsonRpcRequest>(
            &serde_json::json!({
                "id": 5,
                "jsonrpc": "1.0",
                "method": GET_SIGNER_RECEIVE_ADDRESS_METHOD,
            })
            .to_string(),
        )
        .expect("request parse");

        assert_eq!(
            rejected_response(request),
            JsonRpcResponse::Error(JsonRpcErrorResponse {
                id: 5,
                jsonrpc: JsonRpcVersion::V2,
                error: super::JsonRpcErrorObject {
                    code: i32::from(JsonRpcErrorCode::InvalidRequest),
                    message: "unsupported jsonrpc version '1.0'".to_string(),
                },
            })
        );
    }

    #[test]
    fn deserialize_getter_methods_reject_non_empty_params() {
        for method in [
            GET_SIGNER_RECEIVE_ADDRESS_METHOD,
            GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
        ] {
            for params in [
                serde_json::json!([]),
                serde_json::json!({"unexpected": true}),
            ] {
                let request = serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(
                    6,
                    method,
                    Some(params),
                ))
                .expect("request parse");

                assert_eq!(
                    rejected_response(request),
                    JsonRpcResponse::Error(JsonRpcErrorResponse {
                        id: 6,
                        jsonrpc: JsonRpcVersion::V2,
                        error: super::JsonRpcErrorObject {
                            code: i32::from(JsonRpcErrorCode::InvalidParams),
                            message: format!("method '{method}' does not accept params"),
                        },
                    })
                );
            }
        }
    }

    #[test]
    fn deserialize_removed_methods_into_json_rpc_rejection() {
        for method in ["wallet_abi_get_capabilities", "process_request"] {
            let request =
                serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(7, method, None))
                    .expect("request parse");

            assert_eq!(
                rejected_response(request),
                JsonRpcResponse::Error(JsonRpcErrorResponse {
                    id: 7,
                    jsonrpc: JsonRpcVersion::V2,
                    error: super::JsonRpcErrorObject {
                        code: i32::from(JsonRpcErrorCode::MethodNotFound),
                        message: format!("unsupported method '{method}'"),
                    },
                })
            );
        }
    }

    #[test]
    fn deserialize_process_request_rejects_malformed_params() {
        for params in [serde_json::json!({"abi_version": true})] {
            let request = serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(
                8,
                WALLET_ABI_PROCESS_REQUEST_METHOD,
                Some(params),
            ))
            .expect("request parse");

            match rejected_response(request) {
                JsonRpcResponse::Error(JsonRpcErrorResponse { id, jsonrpc, error }) => {
                    assert_eq!(id, 8);
                    assert_eq!(jsonrpc, JsonRpcVersion::V2);
                    assert_eq!(error.code, i32::from(JsonRpcErrorCode::InvalidParams));
                    assert!(error
                        .message
                        .starts_with("invalid 'wallet_abi_process_request' params:"));
                }
                other => panic!("unexpected response: {other:?}"),
            }
        }
    }

    #[test]
    fn deserialize_process_request_rejects_missing_params() {
        let request = serde_json::from_str::<JsonRpcRequest>(&json_rpc_request(
            9,
            WALLET_ABI_PROCESS_REQUEST_METHOD,
            None,
        ))
        .expect("request parse");

        assert_eq!(
            rejected_response(request),
            JsonRpcResponse::Error(JsonRpcErrorResponse {
                id: 9,
                jsonrpc: JsonRpcVersion::V2,
                error: super::JsonRpcErrorObject {
                    code: i32::from(JsonRpcErrorCode::InvalidParams),
                    message: format!(
                        "method '{WALLET_ABI_PROCESS_REQUEST_METHOD}' requires object params"
                    ),
                },
            })
        );
    }

    #[test]
    fn deserialize_malformed_outer_json_returns_error() {
        assert!(serde_json::from_str::<JsonRpcRequest>("{bad-json").is_err());
    }

    #[test]
    fn deserialize_wrong_outer_field_types_returns_error() {
        let request = serde_json::json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": GET_SIGNER_RECEIVE_ADDRESS_METHOD,
        })
        .to_string();

        assert!(serde_json::from_str::<JsonRpcRequest>(&request).is_err());
    }

    #[test]
    fn serialize_getter_responses_as_structured_json_rpc() {
        let response = JsonRpcResponse::result(
            10,
            JsonRpcResultPayload::SignerReceiveAddress(SignerReceiveAddressResult {
                signer_receive_address: sample_signer_receive_address(),
            }),
        );

        let value = serde_json::to_value(&response).expect("serialize response");
        assert_eq!(
            value,
            serde_json::json!({
                "id": 10,
                "jsonrpc": "2.0",
                "result": {
                    "signer_receive_address": "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
                },
            }),
        );

        let x_only_response = JsonRpcResponse::result(
            11,
            JsonRpcResultPayload::RawSigningXOnlyPubkey(RawSigningXOnlyPubkeyResult {
                raw_signing_x_only_pubkey: sample_signing_x_only_pubkey(),
            }),
        );
        let x_only_value = serde_json::to_value(&x_only_response).expect("serialize response");
        assert_eq!(
            x_only_value,
            serde_json::json!({
                "id": 11,
                "jsonrpc": "2.0",
                "result": {
                    "raw_signing_x_only_pubkey": "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                },
            }),
        );
    }

    fn json_rpc_request(id: i64, method: &str, params: Option<serde_json::Value>) -> String {
        let mut request = serde_json::json!({
            "id": id,
            "jsonrpc": "2.0",
            "method": method,
        });

        if let Some(params) = params {
            request["params"] = params;
        }

        request.to_string()
    }

    fn rejected_response(request: JsonRpcRequest) -> JsonRpcResponse {
        match request {
            JsonRpcRequest::Rejected { response } => response,
            other => panic!("expected rejected request, got {other:?}"),
        }
    }

    fn sample_tx_create_request() -> TxCreateRequest {
        TxCreateRequest {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: generate_request_id(),
            network: lwk_common::Network::LocaltestLiquid,
            params: RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            broadcast: false,
        }
    }

    fn sample_signer_receive_address() -> String {
        "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn"
            .to_string()
    }

    fn sample_signing_x_only_pubkey() -> String {
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string()
    }

    #[allow(dead_code)]
    fn _sample_tx_create_response() -> TxCreateResponse {
        TxCreateResponse::error(
            &sample_tx_create_request(),
            &crate::error::WalletAbiError::Funding("insufficient funds".to_string()),
        )
    }
}
