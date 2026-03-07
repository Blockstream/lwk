use serde::{Deserialize, Serialize};

use super::crypto::WalletAbiRelayDirection;

pub(crate) const WALLET_ABI_RELAY_VERSION: u64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalletAbiRelayRequestV2 {
    pub(crate) v: u64,
    pub(crate) pairing_id: String,
    pub(crate) request_id: String,
    pub(crate) origin: String,
    pub(crate) created_at_ms: u64,
    pub(crate) json_rpc_request: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalletAbiRelayResponseError {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalletAbiRelayResponseV2 {
    pub(crate) v: u64,
    pub(crate) pairing_id: String,
    pub(crate) request_id: String,
    pub(crate) origin: String,
    pub(crate) processed_at_ms: u64,
    pub(crate) json_rpc_response: Option<serde_json::Value>,
    pub(crate) error: Option<WalletAbiRelayResponseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayRole {
    Web,
    Phone,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayFrameDirection {
    WebToPhone,
    PhoneToWeb,
}

impl From<WalletAbiRelayFrameDirection> for WalletAbiRelayDirection {
    fn from(value: WalletAbiRelayFrameDirection) -> Self {
        match value {
            WalletAbiRelayFrameDirection::WebToPhone => Self::WebToPhone,
            WalletAbiRelayFrameDirection::PhoneToWeb => Self::PhoneToWeb,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayStatusState {
    PeerConnected,
    RequestSent,
    ResponseSent,
    Closed,
    Expired,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayClientFrameV1 {
    Auth {
        pairing_id: String,
        role: WalletAbiRelayRole,
        token: String,
    },
    Publish {
        pairing_id: String,
        direction: WalletAbiRelayFrameDirection,
        msg_id: String,
        nonce_b64: String,
        ciphertext_b64: String,
    },
    Ack {
        pairing_id: String,
        msg_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalletAbiRelayServerFrameV1 {
    Ack {
        pairing_id: String,
        msg_id: String,
    },
    Deliver {
        pairing_id: String,
        direction: WalletAbiRelayFrameDirection,
        msg_id: String,
        nonce_b64: String,
        ciphertext_b64: String,
        created_at_ms: u64,
    },
    Status {
        pairing_id: String,
        state: WalletAbiRelayStatusState,
        detail: String,
    },
    Error {
        pairing_id: Option<String>,
        code: String,
        message: String,
    },
}
