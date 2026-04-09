use std::sync::Arc;

use crate::{LwkError, Network};
use crate::wallet_abi::abi;

/// Stable provider discovery document for the active wallet/network context.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiCapabilities {
    pub(crate) inner: abi::WalletCapabilities,
}

#[uniffi::export]
impl WalletAbiCapabilities {
    /// Build a capability document from the active network and supported methods.
    #[uniffi::constructor]
    pub fn new(network: &Network, methods: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::WalletCapabilities::new(network.into(), methods),
        })
    }

    /// Parse canonical Wallet ABI capabilities JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize these capabilities to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the ABI version string.
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the active network for this provider instance.
    pub fn network(&self) -> Arc<Network> {
        Arc::new(self.inner.network.into())
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

        let json = capabilities.to_json().expect("serialize capabilities");
        let decoded = WalletAbiCapabilities::from_json(&json).expect("deserialize capabilities");

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
