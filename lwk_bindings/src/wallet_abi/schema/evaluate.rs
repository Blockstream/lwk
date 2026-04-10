use std::sync::Arc;

use crate::wallet_abi::*;

use lwk_simplicity::wallet_abi::schema::tx_create::Status;

/// A typed Wallet ABI transaction evaluation request.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTxEvaluateRequest {
    pub(crate) inner: abi::TxEvaluateRequest,
}

#[uniffi::export]
impl WalletAbiTxEvaluateRequest {
    /// Build a transaction evaluation request.
    ///
    /// `request_id` must be a valid UUID string.
    #[uniffi::constructor]
    pub fn from_parts(
        request_id: &str,
        network: &Network,
        params: &WalletAbiRuntimeParams,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::TxEvaluateRequest::from_parts(
                request_id,
                network.into(),
                params.inner.clone(),
            )?,
        }))
    }

    /// Parse canonical Wallet ABI evaluation request JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this evaluation request to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the ABI version string.
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the request identifier as a UUID string.
    pub fn request_id(&self) -> String {
        self.inner.request_id.to_string()
    }

    /// Return the target network.
    pub fn network(&self) -> Arc<Network> {
        Arc::new(self.inner.network.into())
    }

    /// Return the runtime parameters payload.
    pub fn params(&self) -> Arc<WalletAbiRuntimeParams> {
        Arc::new(WalletAbiRuntimeParams {
            inner: self.inner.params.clone(),
        })
    }
}

/// A typed Wallet ABI transaction evaluation response.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTxEvaluateResponse {
    pub(crate) inner: abi::TxEvaluateResponse,
}

#[uniffi::export]
impl WalletAbiTxEvaluateResponse {
    /// Build a successful transaction evaluation response.
    #[uniffi::constructor]
    pub fn ok(
        request: &WalletAbiTxEvaluateRequest,
        preview: &WalletAbiRequestPreview,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::TxEvaluateResponse::ok(&request.inner, preview.inner.clone()),
        })
    }

    /// Build an error transaction evaluation response.
    #[uniffi::constructor]
    pub fn error(request: &WalletAbiTxEvaluateRequest, error: &WalletAbiErrorInfo) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::TxEvaluateResponse {
                abi_version: abi::TX_CREATE_ABI_VERSION.to_string(),
                request_id: request.inner.request_id,
                network: request.inner.network,
                status: Status::Error,
                preview: None,
                error: Some(error.inner.clone()),
            },
        })
    }

    /// Parse canonical Wallet ABI evaluation response JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this evaluation response to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the ABI version string.
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the request identifier as a UUID string.
    pub fn request_id(&self) -> String {
        self.inner.request_id.to_string()
    }

    /// Return the target network.
    pub fn network(&self) -> Arc<Network> {
        Arc::new(self.inner.network.into())
    }

    /// Return the response status.
    pub fn status(&self) -> WalletAbiStatus {
        self.inner.status.into()
    }

    /// Return the preview payload when this response has `ok` status.
    pub fn preview(&self) -> Option<Arc<WalletAbiRequestPreview>> {
        self.inner.preview.as_ref().map(|preview| {
            Arc::new(WalletAbiRequestPreview {
                inner: preview.clone(),
            })
        })
    }

    /// Return the error payload when this response has `error` status.
    pub fn error_info(&self) -> Option<Arc<WalletAbiErrorInfo>> {
        self.inner.error.as_ref().map(|error| {
            Arc::new(WalletAbiErrorInfo {
                inner: error.clone(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse};
    use crate::blockdata::script::Script;
    use crate::{
        Network, WalletAbiErrorInfo, WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput,
        WalletAbiPreviewOutputKind, WalletAbiRequestPreview, WalletAbiRuntimeParams,
        WalletAbiStatus,
    };

    #[test]
    fn wallet_abi_tx_evaluate_request_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(&[], &[], None, None),
        )
        .expect("request");

        let json = request.to_json().expect("serialize request");
        let decoded = WalletAbiTxEvaluateRequest::from_json(&json).expect("deserialize request");

        assert_eq!(decoded.abi_version(), "wallet-abi-0.1");
        assert_eq!(
            decoded.request_id(),
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14".to_string()
        );
        assert_eq!(decoded.network(), Network::testnet());
        assert!(decoded.params().inputs().is_empty());
        assert!(decoded.params().outputs().is_empty());
        assert_eq!(decoded.params().fee_rate_sat_kvb(), None);
        assert_eq!(decoded.params().lock_time(), None);
    }

    #[test]
    fn wallet_abi_tx_evaluate_response_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(&[], &[], None, None),
        )
        .expect("request");
        let policy_asset = Network::testnet().policy_asset();
        let preview = WalletAbiRequestPreview::new(
            vec![WalletAbiPreviewAssetDelta::new(policy_asset, -1_500)],
            vec![WalletAbiPreviewOutput::new(
                WalletAbiPreviewOutputKind::External,
                policy_asset,
                1_500,
                &Script::empty(),
            )],
            vec!["requires review".to_string()],
        );

        let response = WalletAbiTxEvaluateResponse::ok(&request, &preview);
        let response_json = response.to_json().expect("serialize response");
        let decoded =
            WalletAbiTxEvaluateResponse::from_json(&response_json).expect("deserialize response");

        assert_eq!(decoded.status(), WalletAbiStatus::Ok);
        assert_eq!(
            decoded.preview().expect("preview").warnings(),
            vec!["requires review".to_string()]
        );
        assert!(decoded.error_info().is_none());

        let error = WalletAbiErrorInfo::from_code_string("invalid_request", "bad params", None)
            .expect("error info");
        let error_response = WalletAbiTxEvaluateResponse::error(&request, &error);
        assert_eq!(error_response.status(), WalletAbiStatus::Error);
        assert!(error_response.preview().is_none());
        assert_eq!(
            error_response.error_info().expect("error info").message(),
            "bad params".to_string()
        );
    }
}
