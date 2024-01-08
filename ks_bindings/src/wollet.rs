use crate::desc::WolletDescriptor;
use crate::network::ElementsNetwork;
use crate::types::{AssetId, Hex};
use crate::wallet_tx::WalletTx;
use crate::{Error, Txid};
use common::Signer;
use elements::pset::serialize::Deserialize;
use elements::{
    hex::ToHex,
    pset::{serialize::Serialize, PartiallySignedTransaction},
};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
};

#[derive(uniffi::Object)]
pub struct Wollet {
    inner: Mutex<wollet::Wollet>, // every exposed method must take `&self` (no &mut) so that we need to encapsulate into Mutex
}

#[uniffi::export]
impl Wollet {
    /// Construct a Watch-Only wallet object
    #[uniffi::constructor]
    pub fn new(
        network: ElementsNetwork,
        descriptor: Arc<WolletDescriptor>,
        datadir: String,
    ) -> Result<Arc<Self>, Error> {
        let url = network.electrum_url().to_string();
        let inner = wollet::Wollet::new(
            network.into(),
            &url,
            true,
            true,
            &datadir,
            &descriptor.to_string(),
        )?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    pub fn descriptor(&self) -> Result<String, Error> {
        Ok(self.inner.lock()?.descriptor().to_string())
    }

    pub fn address(&self, index: Option<u32>) -> Result<String, Error> {
        let wollet = self.inner.lock()?;
        let address = wollet.address(index)?;
        Ok(address.address().to_string())
    }

    pub fn sync(&self) -> Result<(), Error> {
        let mut wollet = self.inner.lock()?;
        wollet.sync_tip()?;
        wollet.sync_txs()?;
        Ok(())
    }

    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        Ok(m)
    }

    pub fn transactions(&self) -> Result<Vec<Arc<WalletTx>>, Error> {
        Ok(self
            .inner
            .lock()?
            .transactions()?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    pub fn create_lbtc_tx(
        &self,
        out_address: String,
        satoshis: u64,
        fee_rate: f32,
    ) -> Result<String, Error> {
        let wollet = self.inner.lock()?;
        let pset = wollet.send_lbtc(satoshis, &out_address, Some(fee_rate))?;
        Ok(pset.to_string())
    }

    pub fn sign_tx(&self, mnemonic: String, pset_string: String) -> Result<String, Error> {
        let wollet = self.inner.lock()?;
        let mut pset = match PartiallySignedTransaction::from_str(&pset_string) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        let signer = match signer::SwSigner::new(&mnemonic) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        let _ = signer.sign(&mut pset);
        let tx = match wollet.finalize(&mut pset) {
            Ok(tx) => tx.serialize().to_hex(),
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        Ok(tx)
    }

    pub fn broadcast(&self, tx_hex: Hex) -> Result<Arc<Txid>, Error> {
        let tx = match elements::Transaction::deserialize(tx_hex.as_ref()) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        let wollet = self.inner.lock()?;
        match wollet.broadcast(&tx) {
            Ok(txid) => Ok(Arc::new(txid.into())),
            Err(e) => Err(Error::from(e)),
        }
    }
}
