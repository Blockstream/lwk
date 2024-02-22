use crate::{AddressResult, Error, Network, Update, WalletTx, WolletDescriptor};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Wollet {
    inner: lwk_wollet::Wollet,
}

impl AsRef<lwk_wollet::Wollet> for Wollet {
    fn as_ref(&self) -> &lwk_wollet::Wollet {
        &self.inner
    }
}

impl AsMut<lwk_wollet::Wollet> for Wollet {
    fn as_mut(&mut self) -> &mut lwk_wollet::Wollet {
        &mut self.inner
    }
}

#[wasm_bindgen]
impl Wollet {
    /// Create a new  wallet
    #[wasm_bindgen(constructor)]
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Result<Wollet, Error> {
        let inner = lwk_wollet::Wollet::without_persist(network.into(), descriptor.into())?;
        Ok(Self { inner })
    }

    /// Get a wallet address with the correspondong derivation index
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        // TODO return AddressResult
        let address_result = self.inner.address(index)?;
        Ok(address_result.into())
    }

    pub fn apply_update(&mut self, update: Update) -> Result<(), Error> {
        Ok(self.inner.apply_update(update.into())?)
    }

    pub fn balance(&self) -> Result<JsValue, Error> {
        let balance = self.inner.balance()?;
        Ok(serde_wasm_bindgen::to_value(&balance)?)
    }

    pub fn transactions(&self) -> Result<Vec<WalletTx>, Error> {
        Ok(self
            .inner
            .transactions()?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

#[cfg(test)]
mod tests {

    use crate::{Network, Wollet, WolletDescriptor};
    use lwk_wollet::elements::hex::FromHex;
    use std::collections::HashMap;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    const DESCRIPTOR: &str = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";

    #[wasm_bindgen_test]
    fn test_wollet_address() {
        let descriptor = WolletDescriptor::new(DESCRIPTOR).unwrap();
        let network = Network::mainnet();
        let wollet = Wollet::new(network, descriptor).unwrap();
        assert_eq!(
            wollet.address(Some(0)).unwrap().address().to_string(),
            "VJLAQiChRTcVDXEBKrRnSBnGccJLxNg45zW8cuDwkhbxb8NVFkb4U2QMWAzot4idqhLMWjtZ7SXA4nrA"
        );
    }

    #[ignore = "requires internet connection and takes a while"]
    #[wasm_bindgen_test]
    async fn test_balance_and_transactions() {
        inner_test_balance_and_transactions(true).await;
    }

    #[wasm_bindgen_test]
    async fn test_balance_and_transactions_no_internet() {
        inner_test_balance_and_transactions(false).await;
    }

    async fn inner_test_balance_and_transactions(with_internet: bool) {
        let descriptor = WolletDescriptor::new(DESCRIPTOR).unwrap();
        let network = Network::mainnet();
        let mut wollet = Wollet::new(network, descriptor).unwrap();

        let update = if with_internet {
            let mut client = network.default_esplora_client();
            client.full_scan(&wollet).await.unwrap().unwrap()
        } else {
            let bytes = Vec::<u8>::from_hex(include_str!(
                "../test_data/update_test_balance_and_transactions.hex"
            ))
            .unwrap();
            crate::Update::new(&bytes).unwrap()
        };

        wollet.apply_update(update).unwrap();
        let balance = wollet.balance().unwrap();
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance).unwrap();
        let lbtc = lwk_wollet::ElementsNetwork::Liquid.policy_asset();
        assert!(*balance.get(&lbtc).unwrap() >= 5000);

        let txs = wollet.transactions().unwrap();
        assert!(!txs.is_empty());
        let expected = "b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0";
        assert_eq!(txs[0].txid().to_string(), expected);
        assert_eq!(txs[0].outputs()[0].get().unwrap().unblinded().value(), 5000)
    }
}
