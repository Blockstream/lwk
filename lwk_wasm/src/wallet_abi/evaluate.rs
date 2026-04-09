use super::{json_from_js_value, json_from_str, js_value_from_json};

use crate::{Error, Network, WalletAbiRuntimeParams};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A typed Wallet ABI transaction evaluation request.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiTxEvaluateRequest {
    pub(crate) inner: abi::TxEvaluateRequest,
}

impl WalletAbiTxEvaluateRequest {
    fn from_json_str(json: &str) -> Result<WalletAbiTxEvaluateRequest, Error> {
        json_from_str(json).map(|inner| Self { inner })
    }
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

#[cfg(test)]
mod tests {
    use super::WalletAbiTxEvaluateRequest;

    use crate::{Network, WalletAbiRuntimeParams};

    #[test]
    fn wallet_abi_tx_evaluate_request_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(vec![], vec![], None, None),
        )
        .expect("request");

        let json = request.to_string_js();
        let decoded = WalletAbiTxEvaluateRequest::from_json_str(&json).expect("deserialize");

        assert_eq!(decoded.abi_version(), "wallet-abi-0.1".to_string());
        assert_eq!(
            decoded.request_id(),
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14".to_string()
        );
        assert_eq!(decoded.network(), Network::testnet());
        assert!(decoded.params().inputs().is_empty());
        assert!(decoded.params().outputs().is_empty());
    }
}
