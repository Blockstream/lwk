use crate::Error;
use crate::{ElementsNetwork, WolletDescriptor};

use bitcoincore_rpc::{Auth, Client, RpcApi};

/// A client to issue RPCs to a Elements node
pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    network: ElementsNetwork,
    auth: Auth,
    url: String,
}

impl ElementsRpcClient {
    /// Create a new Elements RPC client
    pub fn new(network: ElementsNetwork, url: &str, auth: Auth) -> Result<Self, Error> {
        let inner = Client::new(url, auth.clone())?;
        Ok(Self {
            inner,
            network,
            auth,
            url: url.to_string(),
        })
    }

    /// Create a new Elements RPC client from credentials
    pub fn new_from_credentials(
        network: ElementsNetwork,
        url: &str,
        user: &str,
        pass: &str,
    ) -> Result<Self, Error> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(network, url, auth)
    }

    /// Get the blockchain height
    pub fn height(&self) -> Result<u64, Error> {
        self.inner
            .call::<serde_json::Value>("getblockcount", &[])?
            .as_u64()
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn("getblockcount".into()))
    }

    fn wallet_client(&self, wallet: &str) -> Result<Self, Error> {
        let url = format!("{0}/wallet/{wallet}", self.url);
        ElementsRpcClient::new(self.network, &url, self.auth.clone())
    }

    fn importdescriptor(&self, desc: &WolletDescriptor, timestamp: u32) -> Result<(), Error> {
        // FIXME: investigate reasonable default for range
        const RANGE: u32 = 10000;

        // importdescriptors requires "bitcoin" descriptors and does not handle multi-path
        let v: Vec<_> = desc
            .single_bitcoin_descriptors()
            .iter()
            .map(|d| ImportDesc {
                desc: d.to_string(),
                range: RANGE,
                timestamp,
            })
            .collect();
        let j = serde_json::to_value(v)?;

        self.inner
            .call::<serde_json::Value>("importdescriptors", &[j])?;

        Ok(())
    }

    /// Create or load a descriptor in the node
    pub fn setup_wallet(
        &self,
        wallet: &str,
        desc: &WolletDescriptor,
        timestamp: u32,
    ) -> Result<Self, Error> {
        // FIXME: if we don't create the wallet, check that the loaded descriptors are consistent

        // Check if the wallet is in listwallets
        let r = self.inner.call::<serde_json::Value>("listwallets", &[])?;
        let wallets: Vec<String> = serde_json::from_value(r)?;
        if wallets.contains(&wallet.to_string()) {
            return self.wallet_client(wallet);
        }

        // Attempt to load the wallet
        if self
            .inner
            .call::<serde_json::Value>("loadwallet", &[wallet.into(), true.into()])
            .is_ok()
        {
            return self.wallet_client(wallet);
        }

        // Create the wallet
        self.inner.call::<serde_json::Value>(
            "createwallet",
            &[
                wallet.into(),
                true.into(),
                true.into(),
                "".into(),
                false.into(),
                true.into(),
            ],
        )?;

        // Import the descriptors
        let wallet_client = self.wallet_client(wallet)?;
        wallet_client.importdescriptor(desc, timestamp)?;

        Ok(wallet_client)
    }

    /// Unload a wallet
    pub fn unloadwallet(&self, name: &str) -> Result<(), Error> {
        self.inner
            .call::<serde_json::Value>("unloadwallet", &[name.into()])?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
struct ImportDesc {
    desc: String,
    range: u32,
    timestamp: u32,
}
