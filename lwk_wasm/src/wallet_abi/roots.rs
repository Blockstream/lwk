use super::{json_from_js_value, json_from_str, js_value_from_json};

use crate::Error;

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// Error details returned by Wallet ABI.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiErrorInfo {
    pub(crate) inner: abi::ErrorInfo,
}

impl WalletAbiErrorInfo {
    fn from_json_str(json: &str) -> Result<WalletAbiErrorInfo, Error> {
        json_from_str(json).map(|inner| Self { inner })
    }
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

#[cfg(test)]
mod tests {
    use super::WalletAbiErrorInfo;

    #[test]
    fn wallet_abi_error_info_roundtrip() {
        let error = WalletAbiErrorInfo::from_code_string(
            "custom_error",
            "boom",
            Some("{\"foo\":1}".to_string()),
        )
        .expect("error info");

        let json = error.to_string_js();
        let decoded = WalletAbiErrorInfo::from_json_str(&json).expect("deserialize");

        assert_eq!(decoded.code_string(), "custom_error".to_string());
        assert_eq!(decoded.message(), "boom".to_string());
        assert_eq!(decoded.details_json().expect("details"), Some("{\"foo\":1}".to_string()));
    }
}
