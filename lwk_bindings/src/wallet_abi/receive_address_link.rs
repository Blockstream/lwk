use std::sync::Arc;

use crate::{Address, LwkError};

use lwk_simplicity::wallet_abi::WalletReceiveAddressProvider;

/// Foreign callback surface for connect-time wallet receive address lookup.
#[uniffi::export(with_foreign)]
pub trait WalletAbiReceiveAddressProviderCallbacks: Send + Sync {
    /// Return the active wallet receive address for the provider session.
    fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError>;
}

/// Error type for the wallet receive-address bridge.
#[derive(thiserror::Error, Debug)]
pub enum WalletReceiveAddressProviderLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
}

/// Bridge adapting foreign receive-address callbacks to runtime `WalletReceiveAddressProvider`.
#[derive(uniffi::Object)]
pub struct WalletReceiveAddressProviderLink {
    inner: Arc<dyn WalletAbiReceiveAddressProviderCallbacks>,
}

#[uniffi::export]
impl WalletReceiveAddressProviderLink {
    /// Create a wallet receive-address bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiReceiveAddressProviderCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

impl WalletReceiveAddressProvider for WalletReceiveAddressProviderLink {
    type Error = WalletReceiveAddressProviderLinkError;

    fn get_signer_receive_address(&self) -> Result<elements::Address, Self::Error> {
        self.inner
            .get_signer_receive_address()
            .map(|address| address.as_ref().into())
            .map_err(|error| WalletReceiveAddressProviderLinkError::Foreign(format!("{error:?}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    struct TestReceiveAddressProviderCallbacks {
        address: Arc<Address>,
    }

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Ok(self.address.clone())
        }
    }

    #[test]
    fn wallet_receive_address_provider_link_adapts_foreign_callbacks() {
        let address = Address::new(
            "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
        )
        .expect("address");
        let link =
            WalletReceiveAddressProviderLink::new(Arc::new(TestReceiveAddressProviderCallbacks {
                address: address.clone(),
            }));

        assert_eq!(
            link.get_signer_receive_address()
                .expect("receive address")
                .to_string(),
            address.to_string()
        );
    }
}
