use super::{json_from_js_value, json_from_str, js_value_from_json};

use crate::{Error, Network};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// Stable provider discovery document for the active wallet/network context.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiCapabilities {
    inner: abi::WalletCapabilities,
}

impl WalletAbiCapabilities {
    fn from_json_str(json: &str) -> Result<WalletAbiCapabilities, Error> {
        json_from_str(json).map(|inner| Self { inner })
    }
}

#[wasm_bindgen]
impl WalletAbiCapabilities {
    /// Build a capability document from the active network and supported methods.
    pub fn new(network: &Network, methods: Vec<String>) -> WalletAbiCapabilities {
        Self {
            inner: abi::WalletCapabilities::new(network.into(), methods),
        }
    }

    /// Parse canonical Wallet ABI capabilities JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiCapabilities, Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize these capabilities to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for these capabilities.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("capabilities contain simple data")
    }

    /// Return the ABI version string.
    #[wasm_bindgen(js_name = abiVersion)]
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the active network for this provider instance.
    pub fn network(&self) -> Network {
        self.inner.network.into()
    }

    /// Return the supported app-facing method names.
    pub fn methods(&self) -> Vec<String> {
        self.inner.methods.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiCapabilities;

    use crate::Network;

    #[test]
    fn wallet_abi_capabilities_roundtrip() {
        let capabilities = WalletAbiCapabilities::new(
            &Network::testnet(),
            vec![
                "wallet_abi_process_request".to_string(),
                "get_signer_receive_address".to_string(),
                "wallet_abi_process_request".to_string(),
            ],
        );

        let json = capabilities.to_string_js();
        let decoded = WalletAbiCapabilities::from_json_str(&json).expect("deserialize");

        assert_eq!(decoded.abi_version(), "wallet-abi-0.1");
        assert_eq!(decoded.network(), Network::testnet());
        assert_eq!(
            decoded.methods(),
            vec![
                "get_signer_receive_address".to_string(),
                "wallet_abi_process_request".to_string(),
            ]
        );
    }
}
