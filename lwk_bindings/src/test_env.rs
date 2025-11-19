use std::sync::Arc;

use crate::{types::AssetId, Address, Txid};

/// Represent a test environment with an elements node and an electrum server.
/// useful for testing only, wrapper over [`lwk_test_util::TestElectrumServer`]
#[derive(uniffi::Object)]
pub struct LwkTestEnv {
    inner: lwk_test_util::TestEnv,
}

#[uniffi::export]
impl LwkTestEnv {
    /// Creates a new test environment
    #[allow(clippy::new_without_default)]
    #[uniffi::constructor]
    pub fn new() -> LwkTestEnv {
        let inner = lwk_test_util::TestEnvBuilder::from_env()
            .with_electrum()
            .build();
        LwkTestEnv { inner }
    }

    /// Generate `blocks` blocks from the node
    pub fn generate(&self, blocks: u32) {
        self.inner.elementsd_generate(blocks);
    }

    /// Get the height of the node
    pub fn height(&self) -> u64 {
        self.inner.elementsd_height()
    }

    /// Send `satoshi` to `address` from the node
    pub fn send_to_address(&self, address: &Address, satoshi: u64, asset: Option<AssetId>) -> Txid {
        self.inner
            .elementsd_sendtoaddress(address.as_ref(), satoshi, asset.map(Into::into))
            .into()
    }

    /// Issue `satoshi` of an asset from the node
    pub fn issue_asset(&self, satoshi: u64) -> AssetId {
        self.inner.elementsd_issueasset(satoshi).into()
    }

    /// Get a new address from the node
    pub fn get_new_address(&self) -> Arc<Address> {
        Arc::new(self.inner.elementsd_getnewaddress().into())
    }

    /// Get the Electrum URL of the test environment
    pub fn electrum_url(&self) -> String {
        self.inner.electrum_url()
    }
}
