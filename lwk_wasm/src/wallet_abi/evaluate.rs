use super::{js_value_from_json, json_from_js_value};

use crate::{
    Error, Network, WalletAbiErrorInfo, WalletAbiRequestPreview, WalletAbiRuntimeParams,
    WalletAbiStatus,
};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A typed Wallet ABI transaction evaluation request.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiTxEvaluateRequest {
    pub(crate) inner: abi::TxEvaluateRequest,
}

#[wasm_bindgen]
impl WalletAbiTxEvaluateRequest {
    /// Build a transaction evaluation request.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        request_id: &str,
        network: &Network,
        params: &WalletAbiRuntimeParams,
    ) -> Result<WalletAbiTxEvaluateRequest, Error> {
        abi::TxEvaluateRequest::from_parts(request_id, network.into(), params.clone().inner)
            .map(|inner| Self { inner })
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Parse canonical Wallet ABI evaluation request JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiTxEvaluateRequest, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this evaluation request to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this request.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("evaluate request contains simple data")
    }

    /// Return the ABI version string.
    #[wasm_bindgen(js_name = abiVersion)]
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the request identifier as a UUID string.
    #[wasm_bindgen(js_name = requestId)]
    pub fn request_id(&self) -> String {
        self.inner.request_id.to_string()
    }

    /// Return the target network.
    pub fn network(&self) -> Network {
        self.inner.network.into()
    }

    /// Return the runtime parameters payload.
    pub fn params(&self) -> WalletAbiRuntimeParams {
        WalletAbiRuntimeParams {
            inner: self.inner.params.clone(),
        }
    }
}

/// A typed Wallet ABI transaction evaluation response.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiTxEvaluateResponse {
    pub(crate) inner: abi::TxEvaluateResponse,
}

#[wasm_bindgen]
impl WalletAbiTxEvaluateResponse {
    /// Build a successful transaction evaluation response.
    pub fn ok(
        request: &WalletAbiTxEvaluateRequest,
        preview: &WalletAbiRequestPreview,
    ) -> WalletAbiTxEvaluateResponse {
        Self {
            inner: abi::TxEvaluateResponse::ok(&request.inner, preview.clone().inner),
        }
    }

    /// Build an error transaction evaluation response.
    pub fn error(
        request: &WalletAbiTxEvaluateRequest,
        error: &WalletAbiErrorInfo,
    ) -> WalletAbiTxEvaluateResponse {
        Self {
            inner: abi::TxEvaluateResponse {
                abi_version: abi::TX_CREATE_ABI_VERSION.to_string(),
                request_id: request.inner.request_id,
                network: request.inner.network,
                status: abi::tx_create::Status::Error,
                preview: None,
                error: Some(error.clone().inner),
            },
        }
    }

    /// Parse canonical Wallet ABI evaluation response JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiTxEvaluateResponse, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this evaluation response to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this response.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("evaluate response contains simple data")
    }

    /// Return the ABI version string.
    #[wasm_bindgen(js_name = abiVersion)]
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the request identifier as a UUID string.
    #[wasm_bindgen(js_name = requestId)]
    pub fn request_id(&self) -> String {
        self.inner.request_id.to_string()
    }

    /// Return the target network.
    pub fn network(&self) -> Network {
        self.inner.network.into()
    }

    /// Return the response status.
    pub fn status(&self) -> WalletAbiStatus {
        self.inner.status.into()
    }

    /// Return the preview payload when this response has `ok` status.
    pub fn preview(&self) -> Option<WalletAbiRequestPreview> {
        self.inner
            .preview
            .as_ref()
            .cloned()
            .map(|inner| WalletAbiRequestPreview { inner })
    }

    /// Return the error payload when this response has `error` status.
    #[wasm_bindgen(js_name = errorInfo)]
    pub fn error_info(&self) -> Option<WalletAbiErrorInfo> {
        self.inner
            .error
            .as_ref()
            .cloned()
            .map(|inner| WalletAbiErrorInfo { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse};

    use crate::{
        Network, Script, WalletAbiErrorInfo, WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput,
        WalletAbiPreviewOutputKind, WalletAbiRequestPreview, WalletAbiRuntimeParams,
        WalletAbiStatus,
    };

    #[test]
    fn wallet_abi_tx_evaluate_request_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(vec![], vec![], None, None),
        )
        .expect("request");

        let json = request.to_string_js();
        let decoded = WalletAbiTxEvaluateRequest {
            inner: serde_json::from_str(&json).expect("deserialize"),
        };

        assert_eq!(decoded.abi_version(), "wallet-abi-0.1".to_string());
        assert_eq!(
            decoded.request_id(),
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14".to_string()
        );
        assert_eq!(decoded.network(), Network::testnet());
        assert!(decoded.params().inputs().is_empty());
        assert!(decoded.params().outputs().is_empty());
    }

    #[test]
    fn wallet_abi_tx_evaluate_response_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(vec![], vec![], None, None),
        )
        .expect("request");
        let policy_asset = Network::testnet().policy_asset();
        let preview = WalletAbiRequestPreview::new(
            vec![WalletAbiPreviewAssetDelta::new(&policy_asset, -1_500)],
            vec![WalletAbiPreviewOutput::new(
                WalletAbiPreviewOutputKind::External,
                &policy_asset,
                1_500,
                &Script::empty(),
            )],
            vec!["requires review".to_string()],
        );

        let response = WalletAbiTxEvaluateResponse::ok(&request, &preview);
        let json = response.to_string_js();
        let decoded = WalletAbiTxEvaluateResponse {
            inner: serde_json::from_str(&json).expect("deserialize"),
        };

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
