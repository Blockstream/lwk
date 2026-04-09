use crate::wallet_abi::TX_CREATE_ABI_VERSION;

use serde::{Deserialize, Serialize};

use lwk_wollet::ElementsNetwork;

/// Stable provider discovery document for the active wallet/network context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WalletCapabilities {
    /// Wallet ABI contract version supported by the provider.
    pub abi_version: String,
    /// Active Elements network for this provider instance.
    pub network: ElementsNetwork,
    /// App-facing method names supported by the provider.
    pub methods: Vec<String>,
}

impl WalletCapabilities {
    /// Build a capability document from the active network and supported methods.
    pub fn new<I, S>(network: ElementsNetwork, methods: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut methods = methods.into_iter().map(Into::into).collect::<Vec<_>>();
        methods.sort();
        methods.dedup();

        Self {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            network,
            methods,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WalletCapabilities;

    #[test]
    fn wallet_capabilities_roundtrip() {
        let capabilities = WalletCapabilities::new(
            lwk_wollet::ElementsNetwork::LiquidTestnet,
            [
                "wallet_abi_process_request",
                "get_signer_receive_address",
                "wallet_abi_process_request",
            ],
        );

        let json = serde_json::to_string(&capabilities).expect("serialize capabilities");
        let decoded: WalletCapabilities =
            serde_json::from_str(&json).expect("deserialize capabilities");

        assert_eq!(decoded.abi_version, "wallet-abi-0.1");
        assert_eq!(
            decoded.methods,
            vec![
                "get_signer_receive_address".to_string(),
                "wallet_abi_process_request".to_string()
            ]
        );
        assert_eq!(decoded, capabilities);
    }
}
