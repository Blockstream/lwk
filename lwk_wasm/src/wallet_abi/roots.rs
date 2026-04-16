use super::{js_value_from_json, json_from_js_value};

use crate::{Error, Network, Txid, WalletAbiRequestPreview, WalletAbiRuntimeParams};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// Error details returned by Wallet ABI.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiErrorInfo {
    pub(crate) inner: abi::ErrorInfo,
}

#[wasm_bindgen]
impl WalletAbiErrorInfo {
    /// Build error info from a canonical Wallet ABI error code string.
    #[wasm_bindgen(js_name = fromCodeString)]
    pub fn from_code_string(
        code: &str,
        message: &str,
        details_json: Option<String>,
    ) -> Result<WalletAbiErrorInfo, Error> {
        abi::ErrorInfo::from_code_and_json(code, message, details_json.as_deref())
            .map(|inner| Self { inner })
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Parse canonical Wallet ABI error JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiErrorInfo, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this error payload to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this error payload.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("error info contains simple data")
    }

    /// Return the canonical Wallet ABI error code string.
    #[wasm_bindgen(js_name = codeString)]
    pub fn code_string(&self) -> String {
        self.inner.code.as_str().to_string()
    }

    /// Return the human-readable error message.
    pub fn message(&self) -> String {
        self.inner.message.clone()
    }

    /// Returns canonical JSON for the open-ended `details` payload.
    #[wasm_bindgen(js_name = detailsJson)]
    pub fn details_json(&self) -> Result<Option<String>, Error> {
        self.inner
            .details_json()
            .map_err(|error| Error::Generic(error.to_string()))
    }
}

/// A typed Wallet ABI transaction creation request.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiTxCreateRequest {
    pub(crate) inner: abi::TxCreateRequest,
}

#[wasm_bindgen]
impl WalletAbiTxCreateRequest {
    /// Build a transaction creation request.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        request_id: &str,
        network: &Network,
        params: &WalletAbiRuntimeParams,
        broadcast: bool,
    ) -> Result<WalletAbiTxCreateRequest, Error> {
        abi::TxCreateRequest::from_parts(
            request_id,
            network.into(),
            params.clone().inner,
            broadcast,
        )
        .map(|inner| Self { inner })
        .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Parse canonical Wallet ABI request JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiTxCreateRequest, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this request to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this request.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("request contains simple data")
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

    /// Return whether the request asks for broadcast.
    pub fn broadcast(&self) -> bool {
        self.inner.broadcast
    }
}

/// The status of a Wallet ABI transaction creation response.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiStatus {
    /// The request succeeded.
    Ok,
    /// The request failed.
    Error,
}

impl From<abi::tx_create::Status> for WalletAbiStatus {
    fn from(value: abi::tx_create::Status) -> Self {
        match value {
            abi::tx_create::Status::Ok => Self::Ok,
            abi::tx_create::Status::Error => Self::Error,
        }
    }
}

/// A created transaction payload returned by Wallet ABI.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiTransactionInfo {
    pub(crate) inner: abi::TransactionInfo,
}

#[wasm_bindgen]
impl WalletAbiTransactionInfo {
    /// Build transaction info from transaction hex and txid.
    pub fn new(tx_hex: &str, txid: &Txid) -> WalletAbiTransactionInfo {
        Self {
            inner: abi::TransactionInfo {
                tx_hex: tx_hex.to_string(),
                txid: (*txid).into(),
            },
        }
    }

    /// Return the transaction hex.
    #[wasm_bindgen(js_name = txHex)]
    pub fn tx_hex(&self) -> String {
        self.inner.tx_hex.clone()
    }

    /// Return the transaction id.
    pub fn txid(&self) -> Txid {
        self.inner.txid.into()
    }
}

/// A typed Wallet ABI transaction creation response.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiTxCreateResponse {
    pub(crate) inner: abi::TxCreateResponse,
}

#[wasm_bindgen]
impl WalletAbiTxCreateResponse {
    /// Build a successful transaction creation response.
    pub fn ok(
        request_id: &str,
        network: &Network,
        transaction: &WalletAbiTransactionInfo,
        artifacts_json: Option<String>,
    ) -> Result<WalletAbiTxCreateResponse, Error> {
        abi::TxCreateResponse::ok_from_parts(
            request_id,
            network.into(),
            transaction.clone().inner,
            artifacts_json.as_deref(),
        )
        .map(|inner| Self { inner })
        .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Build an error transaction creation response.
    pub fn error(
        request_id: &str,
        network: &Network,
        error: &WalletAbiErrorInfo,
    ) -> Result<WalletAbiTxCreateResponse, Error> {
        abi::TxCreateResponse::error_from_parts(request_id, network.into(), error.clone().inner)
            .map(|inner| Self { inner })
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Parse canonical Wallet ABI response JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiTxCreateResponse, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this response to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this response.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("response contains simple data")
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

    /// Return the transaction when this response has `ok` status.
    pub fn transaction(&self) -> Option<WalletAbiTransactionInfo> {
        self.inner
            .transaction
            .as_ref()
            .cloned()
            .map(|inner| WalletAbiTransactionInfo { inner })
    }

    /// Returns canonical JSON for the open-ended `artifacts` payload.
    #[wasm_bindgen(js_name = artifactsJson)]
    pub fn artifacts_json(&self) -> Result<Option<String>, Error> {
        self.inner
            .artifacts_json()
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Return the typed preview payload when `artifacts.preview` is present.
    pub fn preview(&self) -> Result<Option<WalletAbiRequestPreview>, Error> {
        self.inner
            .preview()
            .map(|preview| preview.map(|inner| WalletAbiRequestPreview { inner }))
            .map_err(|error| Error::Generic(error.to_string()))
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
    use super::{
        WalletAbiErrorInfo, WalletAbiStatus, WalletAbiTransactionInfo, WalletAbiTxCreateRequest,
        WalletAbiTxCreateResponse,
    };

    use crate::{
        Network, Script, Txid, WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput,
        WalletAbiPreviewOutputKind, WalletAbiRequestPreview, WalletAbiRuntimeParams,
    };

    #[test]
    fn wallet_abi_error_info_roundtrip() {
        let error = WalletAbiErrorInfo::from_code_string(
            "custom_error",
            "boom",
            Some("{\"foo\":1}".to_string()),
        )
        .expect("error info");

        let json = error.to_string_js();
        let decoded = WalletAbiErrorInfo {
            inner: serde_json::from_str(&json).expect("deserialize"),
        };

        assert_eq!(decoded.code_string(), "custom_error".to_string());
        assert_eq!(decoded.message(), "boom".to_string());
        assert_eq!(
            decoded.details_json().expect("details"),
            Some("{\"foo\":1}".to_string())
        );
    }

    #[test]
    fn wallet_abi_tx_create_request_roundtrip() {
        let request = WalletAbiTxCreateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(vec![], vec![], None, None),
            true,
        )
        .expect("request");

        let json = request.to_string_js();
        let decoded = WalletAbiTxCreateRequest {
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
        assert!(decoded.broadcast());
    }

    #[test]
    fn wallet_abi_tx_create_response_roundtrip() {
        let network = Network::testnet();
        let policy_asset = network.policy_asset();
        let preview = WalletAbiRequestPreview::new(
            vec![WalletAbiPreviewAssetDelta::new(&policy_asset, -1_500)],
            vec![WalletAbiPreviewOutput::new(
                WalletAbiPreviewOutputKind::Fee,
                &policy_asset,
                600,
                &Script::empty(),
            )],
            vec![],
        );
        let transaction = WalletAbiTransactionInfo::new(
            "00",
            &Txid::new("0000000000000000000000000000000000000000000000000000000000000000")
                .expect("txid"),
        );
        let mut artifacts = serde_json::Map::new();
        artifacts.insert(
            "preview".to_string(),
            serde_json::from_str::<serde_json::Value>(&preview.to_string_js())
                .expect("preview json"),
        );
        let response = WalletAbiTxCreateResponse::ok(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &network,
            &transaction,
            Some(serde_json::to_string(&artifacts).expect("artifacts json")),
        )
        .expect("response");

        let json = response.to_string_js();
        let decoded = WalletAbiTxCreateResponse {
            inner: serde_json::from_str(&json).expect("deserialize"),
        };

        assert_eq!(decoded.status(), WalletAbiStatus::Ok);
        assert_eq!(
            decoded.transaction().expect("transaction").tx_hex(),
            "00".to_string()
        );
        assert_eq!(
            decoded
                .preview()
                .expect("preview accessor")
                .expect("preview payload")
                .outputs()[0]
                .kind(),
            WalletAbiPreviewOutputKind::Fee
        );
        assert!(decoded.error_info().is_none());
    }
}
