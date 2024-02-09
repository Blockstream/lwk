use std::sync::Arc;

use crate::{types::AssetId, Address, Txid};

#[derive(uniffi::Object)]
pub struct TestEnv {
    inner: lwk_test_util::TestElectrumServer,
}

#[uniffi::export]
impl TestEnv {
    #[allow(clippy::new_without_default)]
    #[uniffi::constructor]
    pub fn new() -> TestEnv {
        TestEnv {
            inner: lwk_test_util::setup(true),
        }
    }

    pub fn generate(&self, blocks: u32) {
        self.inner.generate(blocks);
    }

    pub fn height(&self) -> u64 {
        self.inner.node_height()
    }

    pub fn send_to_address(&self, address: &Address, satoshi: u64, asset: Option<AssetId>) -> Txid {
        self.inner
            .node_sendtoaddress(address.as_ref(), satoshi, asset.map(Into::into))
            .into()
    }

    pub fn issue_asset(&self, satoshi: u64) -> AssetId {
        self.inner.node_issueasset(satoshi).into()
    }

    pub fn get_new_address(&self) -> Arc<Address> {
        Arc::new(self.inner.node_getnewaddress().into())
    }

    pub fn electrum_url(&self) -> String {
        self.inner.electrs.electrum_url.clone()
    }
}
