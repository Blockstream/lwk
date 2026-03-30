use crate::error::WalletAbiError;
use crate::wallet_abi::schema::runtime_params::RuntimeParams;
use crate::wallet_abi::schema::types::ErrorInfo;

use lwk_wollet::elements::Txid;

use serde::{Deserialize, Serialize};

use lwk_common::Network;

use uuid::Uuid;

pub const TX_CREATE_ABI_VERSION: &str = "wallet-abi-0.1";

/// Generate a fresh canonical request identifier.
pub fn generate_request_id() -> Uuid {
    Uuid::new_v4()
}

/// Transaction-create request envelope for the `wallet-abi-0.1`.
///
/// Security notes:
/// - `abi_version` and `network` are anti-confusion guards and must be validated
///   before wallet/network side effects.
/// - `request_id` is for correlation and tracing, not replay protection.
/// - `broadcast = false` means "do not publish transaction", not "do not touch network":
///   runtime may still perform wallet sync and UTXO fetches.
///
/// UX guidance:
/// - generate a fresh `request_id` per user action,
/// - preserve `request_id` across transport/relay/response surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TxCreateRequest {
    /// ABI contract version.
    ///
    /// Must equal [`TX_CREATE_ABI_VERSION`].
    pub abi_version: String,
    /// Correlation identifier for the request.
    pub request_id: Uuid,
    /// Target Elements network for this request.
    pub network: Network,
    /// Transaction construction parameters to be consumed by runtime.
    pub params: RuntimeParams,
    /// Broadcast policy for runtime.
    ///
    /// - `true`: publish transaction through runtime's configured broadcaster.
    /// - `false`: build/finalize only.
    pub broadcast: bool,
}

impl TxCreateRequest {
    /// Validate request-level contract fields against the active runtime context.
    ///
    /// # Errors
    ///
    /// Returns [`WalletAbiError::InvalidRequest`] when `abi_version` or `network`
    /// does not match runtime expectations.
    pub fn validate_for_runtime(&self, runtime_network: Network) -> Result<(), WalletAbiError> {
        if self.abi_version != TX_CREATE_ABI_VERSION {
            return Err(WalletAbiError::InvalidRequest(format!(
                "request abi_version mismatch: expected '{TX_CREATE_ABI_VERSION}', got '{}'",
                self.abi_version
            )));
        }

        if self.network != runtime_network {
            return Err(WalletAbiError::InvalidRequest(format!(
                "request network mismatch: expected {:?}, got {:?}",
                runtime_network, self.network
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionInfo {
    /// Fully signed Elements transaction serialized as lowercase hex.
    ///
    /// Security:
    /// treat this as public information that can be retrieved from a chain
    pub tx_hex: String,
    /// Transaction identifier for `tx_hex`.
    pub txid: Txid,
}

/// Optional response extension map for producer-specific metadata.
///
/// Compatibility:
/// keys are open-ended by design. Consumers should ignore unknown keys.
pub type TxCreateArtifacts = serde_json::Map<String, serde_json::Value>;

/// High-level outcome status for [`TxCreateResponse`].
///
/// Canonical values are serialized as `snake_case` strings:
/// `ok` and `error`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Transaction creation completed successfully.
    Ok,
    /// Transaction creation failed.
    Error,
}

/// Transaction-create response envelope for the `wallet-abi-0.1` ABI.
///
/// Producer nuance:
/// - Core runtime entrypoints return `Result<TxCreateResponse, WalletAbiError>` and
///   produce successful envelopes via [`TxCreateResponse::ok`].
/// - Adapter layers (for example `UniFFI` bindings) may normalize runtime/business failures
///   into ABI error envelopes via [`TxCreateResponse::error`].
///
/// Security notes:
/// - `request_id` is correlation-critical and should be preserved end-to-end.
/// - `transaction.tx_hex`, `error.message`, `error.details`, and `artifacts` should
///   be treated as public content.
///
/// UX guidance:
/// - Distinguish clearly between "transaction built" and "transaction broadcast".
/// - Prefer machine branching on `error.code`; use `error.message` as technical context.
/// - Surface both `network` and `request_id` in UI/logs to avoid cross-request confusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxCreateResponse {
    /// ABI contract version for this envelope.
    pub abi_version: String,
    /// Correlation identifier copied from the originating request.
    pub request_id: Uuid,
    /// Network context for this response.
    pub network: Network,
    /// Outcome status for the request.
    pub status: Status,
    /// Successful transaction payload (`Some` on `status = ok`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<TransactionInfo>,
    /// Optional producer-specific metadata extension map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<TxCreateArtifacts>,
    /// Structured error payload (`Some` on `status = error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

impl TxCreateResponse {
    /// Build a successful ABI response envelope.
    pub fn ok(
        request: &TxCreateRequest,
        transaction: TransactionInfo,
        artifacts: Option<TxCreateArtifacts>,
    ) -> Self {
        Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: request.request_id,
            network: request.network,
            status: Status::Ok,
            transaction: Some(transaction),
            artifacts,
            error: None,
        }
    }

    /// Build an error ABI response envelope.
    ///
    /// Intended for transport/adapters that must always return ABI responses
    /// instead of bubbling runtime errors.
    pub fn error(request: &TxCreateRequest, error: &WalletAbiError) -> Self {
        Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: request.request_id,
            network: request.network,
            status: Status::Error,
            transaction: None,
            artifacts: None,
            error: Some(error.into()),
        }
    }
}
