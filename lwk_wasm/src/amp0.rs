use crate::{AddressResult, Error, Network, Transaction, WebSocketSerial};

use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::amp0::Amp0`]
#[wasm_bindgen]
pub struct Amp0 {
    inner: lwk_wollet::amp0::Amp0<WebSocketSerial>,
    network: Network,
}

#[wasm_bindgen]
impl Amp0 {
    /// Create a new AMP0 context for the specified network
    #[wasm_bindgen(js_name = newWithNetwork)]
    pub async fn new_with_network(
        network: &Network,
        username: &str,
        password: &str,
        amp_id: &str,
    ) -> Result<Self, Error> {
        let url = lwk_wollet::amp0::default_url((*network).into())?;
        let websocket_serial = WebSocketSerial::new_wamp(url).await?;
        let inner = lwk_wollet::amp0::Amp0::new(
            websocket_serial,
            (*network).into(),
            username,
            password,
            amp_id,
        )
        .await?;
        Ok(Self {
            inner,
            network: *network,
        })
    }

    /// Create a new AMP0 context for testnet
    #[wasm_bindgen(js_name = newTestnet)]
    pub async fn new_testnet(username: &str, password: &str, amp_id: &str) -> Result<Self, Error> {
        Self::new_with_network(&Network::testnet(), username, password, amp_id).await
    }

    /// Create a new AMP0 context for mainnet
    #[wasm_bindgen(js_name = newMainnet)]
    pub async fn new_mainnet(username: &str, password: &str, amp_id: &str) -> Result<Self, Error> {
        Self::new_with_network(&Network::mainnet(), username, password, amp_id).await
    }

    /// Index of the last returned address
    ///
    /// Use this and [`crate::EsploraClient::full_scan_to_index()`] to sync the `Wollet`
    #[wasm_bindgen(js_name = lastIndex)]
    pub fn last_index(&self) -> u32 {
        self.inner.last_index()
    }

    /// AMP ID
    #[wasm_bindgen(js_name = ampId)]
    pub fn amp_id(&self) -> String {
        self.inner.amp_id().into()
    }

    /// Get an address
    ///
    /// If `index` is None, a new address is returned.
    pub async fn address(&mut self, index: Option<u32>) -> Result<AddressResult, Error> {
        let address_result = self.inner.address(index).await?;
        Ok(address_result.into())
    }

    /// The LWK watch-only wallet corresponding to the AMP0 (sub)account.
    pub fn wollet(&self) -> Result<crate::Wollet, Error> {
        Ok(crate::Wollet::new(
            &self.network,
            &self.inner.wollet_descriptor().into(),
        )?)
    }

    /// Ask AMP0 server to cosign
    pub async fn sign(&self, amp0pset: &Amp0Pset) -> Result<Transaction, Error> {
        let tx = self.inner.sign(amp0pset.as_ref()).await?;
        Ok(tx.into())
    }
}

/// Wrapper of [`lwk_wollet::amp0::Amp0Pset`]
#[wasm_bindgen]
pub struct Amp0Pset {
    inner: lwk_wollet::amp0::Amp0Pset,
}

impl From<lwk_wollet::amp0::Amp0Pset> for Amp0Pset {
    fn from(inner: lwk_wollet::amp0::Amp0Pset) -> Self {
        Self { inner }
    }
}

impl From<Amp0Pset> for lwk_wollet::amp0::Amp0Pset {
    fn from(pset: Amp0Pset) -> Self {
        pset.inner
    }
}

impl AsRef<lwk_wollet::amp0::Amp0Pset> for Amp0Pset {
    fn as_ref(&self) -> &lwk_wollet::amp0::Amp0Pset {
        &self.inner
    }
}

#[wasm_bindgen]
impl Amp0Pset {
    /// Creates a `Amp0Pset`
    #[wasm_bindgen(constructor)]
    pub fn new(pset: crate::Pset, blinding_nonces: Vec<String>) -> Result<Self, Error> {
        let inner = lwk_wollet::amp0::Amp0Pset::new(pset.into(), blinding_nonces)?;
        Ok(Self { inner })
    }

    /// Get the PSET
    pub fn pset(&self) -> crate::Pset {
        self.inner.pset().clone().into()
    }

    /// Get the blinding nonces
    #[wasm_bindgen(js_name = blindingNonces)]
    pub fn blinding_nonces(&self) -> Vec<String> {
        self.inner.blinding_nonces().to_vec()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp0ext() {
        let mut amp0 = Amp0::new_mainnet("userleo456", "userleo456", "")
            .await
            .unwrap();
        let last_index = amp0.last_index();
        assert!(last_index > 20);

        let addr = amp0.address(None).await.unwrap();
        assert_eq!(addr.index(), last_index + 1);
        assert_eq!(amp0.last_index(), last_index + 1);

        // Sync the wollet
        let mut wollet = amp0.wollet().unwrap();

        let network = Network::mainnet();
        let mut client = network.default_esplora_client();
        let update = client.full_scan(&wollet).await.unwrap().unwrap();

        wollet.apply_update(&update).unwrap();
        let balance = wollet.balance().unwrap();
        use std::collections::HashMap;
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance).unwrap();
        let lbtc = lwk_wollet::ElementsNetwork::Liquid.policy_asset();
        let mut expected = HashMap::new();
        expected.insert(lbtc, 1000);
        assert_eq!(balance, expected);
    }

    #[wasm_bindgen_test]
    #[ignore] // Takes too long
    async fn test_amp0_e2e() {
        use crate::{Mnemonic, Signer, TxBuilder};
        use std::collections::HashMap;

        let network = Network::testnet();
        let mut amp0 = Amp0::new_testnet("userleo345678", "userleo345678", "")
            .await
            .unwrap();

        // Get an address
        let addr = amp0.address(Some(1)).await.unwrap().address();

        // Create wollet
        let mut wollet = amp0.wollet().unwrap();

        // Sync the wollet
        let mut client = network.default_esplora_client();
        let last_index = amp0.last_index();
        let update = client
            .full_scan_to_index(&wollet, last_index)
            .await
            .unwrap()
            .unwrap();
        wollet.apply_update(&update).unwrap();

        // Get balance
        let balance = wollet.balance().unwrap();
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance).unwrap();
        let lbtc = lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset();
        let lbtc_balance = balance.get(&lbtc).unwrap_or(&0);
        if *lbtc_balance < 500 {
            println!(
                "Balance is insufficient to make a transaction, send some tLBTC to {}",
                addr
            );
            return;
        }

        // Create transaction
        let amp0pset = TxBuilder::new(&network)
            .drain_lbtc_wallet()
            .finish_for_amp0(&wollet)
            .unwrap();

        // User signs
        let mnemonic = Mnemonic::new(
            "student lady today genius gentle zero satoshi book just link gauge tooth",
        )
        .unwrap();
        let signer = Signer::new(&mnemonic, &network).unwrap();
        let pset = signer.sign(amp0pset.pset()).unwrap();

        // Amp0 cosign
        let amp0pset = Amp0Pset::new(pset, amp0pset.blinding_nonces()).unwrap();
        let tx = amp0.sign(&amp0pset).await.unwrap();
        // Broadcast
        let _txid = client.broadcast_tx(&tx).await.unwrap();
    }
}
