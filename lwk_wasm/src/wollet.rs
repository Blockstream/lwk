use crate::blockdata::asset_id::AssetIds;
use crate::{
    AddressResult, Error, Network, Pset, PsetDetails, Transaction, Update, WalletTx, WalletTxOut,
    WolletDescriptor,
};
use lwk_jade::derivation_path_to_vec;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements_miniscript::ForEachKey;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::Wollet`]
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
    /// Create a `Wollet`
    #[wasm_bindgen(constructor)]
    pub fn new(network: &Network, descriptor: &WolletDescriptor) -> Result<Wollet, Error> {
        let inner = lwk_wollet::Wollet::without_persist(network.into(), descriptor.into())?;
        Ok(Self { inner })
    }

    /// Get a wallet address with the correspondong derivation index
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        let address_result = self.inner.address(index)?;
        Ok(address_result.into())
    }

    #[wasm_bindgen(js_name = addressFullPath)]
    pub fn address_full_path(&self, index: u32) -> Result<Vec<u32>, Error> {
        // TODO we should add the full path to lwk_wollet::AddressResult

        let definite_desc = self
            .inner
            .wollet_descriptor()
            .definite_descriptor(lwk_wollet::Chain::External, index)?;
        let mut full_path: Vec<u32> = vec![];
        definite_desc.for_each_key(|k| {
            if let Some(path) = k.full_derivation_path() {
                full_path = derivation_path_to_vec(&path);
            }

            true
        });

        Ok(full_path)
    }

    #[wasm_bindgen(js_name = applyUpdate)]
    pub fn apply_update(&mut self, update: &Update) -> Result<(), Error> {
        Ok(self.inner.apply_update(update.into())?)
    }

    #[wasm_bindgen(js_name = applyTransaction)]
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), Error> {
        Ok(self.inner.apply_transaction(tx.clone().into())?)
    }

    pub fn balance(&self) -> Result<JsValue, Error> {
        let balance = self.inner.balance()?;
        let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);
        Ok(balance.serialize(&serializer)?)
    }

    #[wasm_bindgen(js_name = assetsOwned)]
    pub fn assets_owned(&self) -> Result<AssetIds, Error> {
        Ok(self.inner.assets_owned()?.into())
    }

    pub fn transactions(&self) -> Result<Vec<WalletTx>, Error> {
        Ok(self
            .inner
            .transactions()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Get the unspent transaction outputs of the wallet
    pub fn utxos(&self) -> Result<Vec<WalletTxOut>, Error> {
        Ok(self.inner.utxos()?.into_iter().map(Into::into).collect())
    }

    /// Get all the transaction outputs of the wallet, both spent and unspent
    pub fn txos(&self) -> Result<Vec<WalletTxOut>, Error> {
        Ok(self.inner.txos()?.into_iter().map(Into::into).collect())
    }

    /// Finalize and consume the given PSET, returning the finalized one
    pub fn finalize(&self, pset: Pset) -> Result<Pset, Error> {
        let mut pset: PartiallySignedTransaction = pset.into();
        self.inner.finalize(&mut pset)?;
        Ok(pset.into())
    }

    #[wasm_bindgen(js_name = psetDetails)]
    pub fn pset_details(&self, pset: &Pset) -> Result<PsetDetails, Error> {
        let pset: PartiallySignedTransaction = pset.clone().into();
        let details = self.inner.get_details(&pset)?;
        Ok(details.into())
    }

    pub fn descriptor(&self) -> Result<WolletDescriptor, Error> {
        Ok(self.inner.wollet_descriptor().into())
    }

    pub fn status(&self) -> u64 {
        self.inner.status()
    }

    pub fn tip(&self) -> Tip {
        self.inner.tip().into()
    }

    /// wraps [lwk_wollet::Wollet::never_scanned()]
    #[wasm_bindgen(js_name = neverScanned)]
    pub fn never_scanned(&self) -> bool {
        self.inner.never_scanned()
    }
}

/// Wrapper of [`lwk_wollet::Tip`]
#[wasm_bindgen]
pub struct Tip {
    inner: lwk_wollet::Tip,
}

impl From<lwk_wollet::Tip> for Tip {
    fn from(inner: lwk_wollet::Tip) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl Tip {
    pub fn height(&self) -> u32 {
        self.inner.height()
    }
    pub fn hash(&self) -> String {
        self.inner.hash().to_string()
    }
    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
#[wasm_bindgen]
pub fn big_number() -> Result<JsValue, Error> {
    use serde::Serialize;
    use serde_wasm_bindgen::Serializer;

    let big = 11988790300000000u64;
    let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);
    Ok(big.serialize(&serializer)?)
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use crate::{Network, Wollet, WolletDescriptor};
    use lwk_wollet::elements::hex::FromHex;
    use std::collections::HashMap;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    const DESCRIPTOR: &str = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";

    #[wasm_bindgen_test]
    fn test_big_number() {
        super::big_number().unwrap();
    }

    #[wasm_bindgen_test]
    fn test_wollet_address() {
        let descriptor = WolletDescriptor::new(DESCRIPTOR).unwrap();
        let network = Network::mainnet();
        let wollet = Wollet::new(&network, &descriptor).unwrap();
        assert_eq!(
            wollet.address(Some(0)).unwrap().address().to_string(),
            "VJLAQiChRTcVDXEBKrRnSBnGccJLxNg45zW8cuDwkhbxb8NVFkb4U2QMWAzot4idqhLMWjtZ7SXA4nrA"
        );
        assert_eq!(wollet.descriptor().unwrap().to_string(), "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f");
        assert_eq!(wollet.status(), 1421324647);
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
        let mut wollet = Wollet::new(&network, &descriptor).unwrap();

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

        wollet.apply_update(&update).unwrap();
        let balance = wollet.balance().unwrap();
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance).unwrap();
        let lbtc = lwk_wollet::ElementsNetwork::Liquid.policy_asset();
        assert!(*balance.get(&lbtc).unwrap() >= 5000);

        let txs = wollet.transactions().unwrap();
        assert!(!txs.is_empty());
        let tx = &txs[0];
        let expected = "b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0";
        assert_eq!(tx.txid().to_string(), expected);
        assert_eq!(tx.outputs()[0].get().unwrap().unblinded().value(), 5000);
        assert_eq!(tx.unblinded_url("https://blockstream.info/liquid/"), "https://blockstream.info/liquid/tx/b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0#blinded=5000,6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d,996317a530241d05658cd5e7cecbaf89a3a23962c944c3459b21964410d08176,1eccf2fd5e8160343f2c69e2e9d261fec783a6f0f942387c0e78b25471672462");

        let utxos = wollet.utxos().unwrap();
        assert!(!utxos.is_empty());
        let utxo = &utxos[0];
        assert_eq!(
            utxo.address().to_string(),
            "VJLAQiChRTcVDXEBKrRnSBnGccJLxNg45zW8cuDwkhbxb8NVFkb4U2QMWAzot4idqhLMWjtZ7SXA4nrA"
        );

        let txos = wollet.txos().unwrap();
        assert!(!txos.is_empty());
        assert!(txos.len() >= utxos.len());
        let txo = &txos[0];
        assert_eq!(
            txo.address().to_string(),
            "VJLAQiChRTcVDXEBKrRnSBnGccJLxNg45zW8cuDwkhbxb8NVFkb4U2QMWAzot4idqhLMWjtZ7SXA4nrA"
        );
    }
}
