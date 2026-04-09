use std::sync::Arc;

use crate::Network;

/// Context shared by Rust-owned wallet-abi signer adapters.
#[derive(uniffi::Record, Clone)]
pub struct WalletAbiSignerContext {
    /// Active LWK network for the account being served.
    pub network: Arc<Network>,
    /// Account index used when deriving wallet-abi signing keys.
    pub account_index: u32,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::WalletAbiSignerContext;
    use crate::Network;

    #[test]
    fn wallet_abi_signer_context_holds_network_and_account() {
        let network = Network::testnet();
        let context = WalletAbiSignerContext {
            network: network.clone(),
            account_index: 7,
        };

        assert!(Arc::ptr_eq(&context.network, &network));
        assert_eq!(context.account_index, 7);
    }
}
