use crate::{AddressResult, Error, Network, Signer, Transaction, WebSocketSerial};
use lwk_common::Amp0Signer;

use wasm_bindgen::prelude::*;

/// Context for actions related to an AMP0 (sub)account
///
/// <div class="warning">
/// <b>WARNING:</b>
///
/// AMP0 is based on a legacy system, and some things do not fit precisely the way LWK allows to do
/// things.
///
/// Callers must be careful with the following:
/// * <b>Addresses: </b>
///   to get addresses use [`Amp0::address()`]. This ensures
///   that all addresses used are correctly monitored by the AMP0 server.
/// * <b>Syncing: </b>
///   to sync the AMP0 [`crate::Wollet`], use [`Amp0::last_index()`] and [`crate::clients::blocking::BlockchainBackend::full_scan_to_index()`]. This ensures that all utxos are synced, even if there are gaps between higher than the GAP LIMIT.
///
/// <i>
/// Failing to do the above might lead to inconsistent states, where funds are not shown or they
/// cannot be spent!
/// </i>
/// </div>
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

/// A PSET to use with AMP0
///
/// When asking AMP0 to cosign, the caller must pass some extra data that does not belong to the
/// PSET. This struct holds and manage the necessary data.
///
/// If you're not dealing with AMP0, do not use this struct.
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

#[wasm_bindgen]
pub struct Amp0SignerData {
    inner: lwk_common::Amp0SignerData,
}

impl std::fmt::Display for Amp0SignerData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}
impl From<lwk_common::Amp0SignerData> for Amp0SignerData {
    fn from(inner: lwk_common::Amp0SignerData) -> Self {
        Self { inner }
    }
}

impl AsRef<lwk_common::Amp0SignerData> for Amp0SignerData {
    fn as_ref(&self) -> &lwk_common::Amp0SignerData {
        &self.inner
    }
}

#[wasm_bindgen]
impl Signer {
    /// AMP0 signer data for login
    #[wasm_bindgen(js_name = amp0SignerData)]
    pub fn amp0_signer_data(&self) -> Result<Amp0SignerData, Error> {
        Ok(self.inner.amp0_signer_data()?.into())
    }

    /// AMP0 sign login challenge
    #[wasm_bindgen(js_name = amp0SignChallenge)]
    pub fn amp0_sign_challenge(&self, challenge: &str) -> Result<String, Error> {
        Ok(self.inner.amp0_sign_challenge(challenge)?)
    }

    /// AMP0 account xpub
    #[wasm_bindgen(js_name = amp0AccountXpub)]
    pub fn amp0_account_xpub(&self, account: u32) -> Result<String, Error> {
        Ok(self.inner.amp0_account_xpub(account)?.to_string())
    }
}

/// Session connecting to AMP0
#[wasm_bindgen]
pub struct Amp0Connected {
    inner: lwk_wollet::amp0::Amp0Connected<WebSocketSerial>,
}

#[wasm_bindgen]
impl Amp0Connected {
    /// Connect and register to AMP0
    #[wasm_bindgen(constructor)]
    pub async fn new(network: &Network, signer_data: &Amp0SignerData) -> Result<Self, Error> {
        let url = lwk_wollet::amp0::default_url((*network).into())?;
        let websocket_serial = WebSocketSerial::new_wamp(url).await?;
        let inner = lwk_wollet::amp0::Amp0Connected::new(
            websocket_serial,
            (*network).into(),
            signer_data.inner.clone(),
        )
        .await?;
        Ok(Self { inner })
    }

    /// Obtain a login challenge
    ///
    /// This must be signed with [`amp0_sign_challenge()`].
    #[wasm_bindgen(js_name = getChallenge)]
    pub async fn get_challenge(&self) -> Result<String, Error> {
        Ok(self.inner.get_challenge().await?)
    }

    /// Log in
    ///
    /// `sig` must be obtained from [`amp0_sign_challenge()`] called with the value returned
    /// by [`Amp0Connected::get_challenge()`]
    pub async fn login(self, sig: &str) -> Result<Amp0LoggedIn, Error> {
        let inner = self.inner.login(sig).await?;
        Ok(Amp0LoggedIn { inner })
    }
}

/// Session logged in AMP0
#[wasm_bindgen]
pub struct Amp0LoggedIn {
    inner: lwk_wollet::amp0::Amp0LoggedIn<WebSocketSerial>,
}

#[wasm_bindgen]
impl Amp0LoggedIn {
    /// List of AMP IDs.
    #[wasm_bindgen(js_name = getAmpIds)]
    pub fn get_amp_ids(&self) -> Result<Vec<String>, Error> {
        Ok(self.inner.get_amp_ids()?)
    }

    /// Get the next account for AMP0 account creation
    ///
    /// This must be given to [`amp0_account_xpub()`] to obtain the xpub to pass to
    /// [`Amp0LoggedIn::create_amp0_account()`]
    #[wasm_bindgen(js_name = nextAccount)]
    pub fn next_account(&self) -> Result<u32, Error> {
        Ok(self.inner.next_account()?)
    }

    /// Create a new AMP0 account
    ///
    /// `account_xpub` must be obtained from [`amp0_account_xpub()`] called with the value obtained from
    /// [`Amp0LoggedIn::next_account()`]
    #[wasm_bindgen(js_name = createAmp0Account)]
    pub async fn create_amp0_account(
        &mut self,
        pointer: u32,
        account_xpub: &str,
    ) -> Result<String, Error> {
        use lwk_wollet::elements::bitcoin::bip32::Xpub;
        use std::str::FromStr;
        let account_xpub = Xpub::from_str(account_xpub)?;
        Ok(self
            .inner
            .create_amp0_account(pointer, &account_xpub)
            .await?)
    }

    /// Create a new Watch-Only entry for this wallet
    #[wasm_bindgen(js_name = createWatchOnly)]
    pub async fn create_watch_only(&mut self, username: &str, password: &str) -> Result<(), Error> {
        Ok(self.inner.create_watch_only(username, password).await?)
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
            serde_wasm_bindgen::from_value(balance.entries().unwrap()).unwrap();
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
        let mut amp0 = Amp0::new_testnet("userlwk001", "userlwk001", "")
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
            serde_wasm_bindgen::from_value(balance.entries().unwrap()).unwrap();
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
            "thrive metal cactus come oval candy medal bounce captain shock permit joke",
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

    #[wasm_bindgen_test]
    #[ignore = "Requires network connectivity and it takes too long"]
    async fn test_amp0_create_account() {
        use crate::{Bip, Mnemonic, Signer};

        let network = Network::testnet();

        let mnemonic = Mnemonic::from_random(12).unwrap();
        let signer = Signer::new(&mnemonic, &network).unwrap();
        let fp = &signer.keyorigin_xpub(&Bip::bip49()).unwrap()[1..9];
        let username = format!("user{}", fp);
        let password = format!("pass{}", fp);

        // Login to AMP0
        let sd = signer.amp0_signer_data().unwrap();
        let amp0 = Amp0Connected::new(&network, &sd).await.unwrap();
        let challenge = amp0.get_challenge().await.unwrap();
        let sig = signer.amp0_sign_challenge(&challenge).unwrap();
        let mut amp0 = amp0.login(&sig).await.unwrap();

        // Create AMP0 account
        let pointer = amp0.next_account().unwrap();
        let xpub = signer.amp0_account_xpub(pointer).unwrap();
        let amp_id = amp0.create_amp0_account(pointer, &xpub).await.unwrap();

        // Create Watch-Only
        amp0.create_watch_only(&username, &password).await.unwrap();

        // Login Watch-Only
        let _amp0 = Amp0::new_testnet(&username, &password, &amp_id)
            .await
            .unwrap();
    }
}
