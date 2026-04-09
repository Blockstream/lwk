use crate::error::WalletAbiError;
use crate::wallet_abi::schema::preview::RequestPreview;
use crate::wallet_abi::schema::runtime_params::RuntimeParams;
use crate::wallet_abi::schema::tx_create::{Status, TX_CREATE_ABI_VERSION};
use crate::wallet_abi::schema::types::ErrorInfo;

use serde::{Deserialize, Serialize};

use lwk_wollet::ElementsNetwork;

use uuid::Uuid;

/// Preflight request envelope for evaluating a wallet-abi transaction request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TxEvaluateRequest {
    /// ABI contract version.
    ///
    /// Must equal [`TX_CREATE_ABI_VERSION`].
    pub abi_version: String,
    /// Correlation identifier for the request.
    pub request_id: Uuid,
    /// Target Elements network for this request.
    pub network: ElementsNetwork,
    /// Transaction construction parameters to be evaluated without broadcast.
    pub params: RuntimeParams,
}

impl TxEvaluateRequest {
    /// Build an evaluation request envelope from primitive parts.
    pub fn from_parts(
        request_id: &str,
        network: ElementsNetwork,
        params: RuntimeParams,
    ) -> Result<Self, WalletAbiError> {
        Ok(Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: request_id.parse().map_err(WalletAbiError::from)?,
            network,
            params,
        })
    }

    /// Validate request-level contract fields against the active runtime context.
    pub fn validate_for_runtime(
        &self,
        runtime_network: ElementsNetwork,
    ) -> Result<(), WalletAbiError> {
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

/// Preflight response envelope for wallet-abi request evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxEvaluateResponse {
    /// ABI contract version for this envelope.
    pub abi_version: String,
    /// Correlation identifier copied from the originating request.
    pub request_id: Uuid,
    /// Network context for this response.
    pub network: ElementsNetwork,
    /// Outcome status for the request.
    pub status: Status,
    /// Successful preview payload (`Some` on `status = ok`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<RequestPreview>,
    /// Structured error payload (`Some` on `status = error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

impl TxEvaluateResponse {
    /// Build a successful evaluation response envelope.
    pub fn ok(request: &TxEvaluateRequest, preview: RequestPreview) -> Self {
        Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: request.request_id,
            network: request.network,
            status: Status::Ok,
            preview: Some(preview),
            error: None,
        }
    }

    /// Build an error evaluation response envelope.
    pub fn error(request: &TxEvaluateRequest, error: &WalletAbiError) -> Self {
        Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: request.request_id,
            network: request.network,
            status: Status::Error,
            preview: None,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TxEvaluateRequest, TxEvaluateResponse};
    use crate::wallet_abi::schema::{
        PreviewAssetDelta, PreviewOutput, PreviewOutputKind, RequestPreview, RuntimeParams,
    };

    #[test]
    fn tx_evaluate_roundtrip() {
        let request = TxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            lwk_wollet::ElementsNetwork::LiquidTestnet,
            RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: Some(123.0),
                lock_time: Some(lwk_wollet::elements::LockTime::from_consensus(42)),
            },
        )
        .expect("request");
        let response = TxEvaluateResponse::ok(
            &request,
            RequestPreview {
                asset_deltas: vec![PreviewAssetDelta {
                    asset_id: lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset(),
                    wallet_delta_sat: -1_500,
                }],
                outputs: vec![PreviewOutput {
                    kind: PreviewOutputKind::External,
                    asset_id: lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset(),
                    amount_sat: 1_500,
                    script_pubkey: lwk_wollet::elements::Script::new(),
                }],
                warnings: vec!["requires review".to_string()],
            },
        );

        let request_json = serde_json::to_string(&request).expect("serialize request");
        let response_json = serde_json::to_string(&response).expect("serialize response");

        assert_eq!(
            serde_json::from_str::<TxEvaluateRequest>(&request_json).expect("deserialize request"),
            request
        );
        assert_eq!(
            serde_json::from_str::<TxEvaluateResponse>(&response_json)
                .expect("deserialize response"),
            response
        );
    }
}
