use crate::desc::WolletDescriptor;
use crate::network::ElementsNetwork;
use crate::tx::{Tx, TxIn, TxOut};
use crate::types::{Hex, Txid};
use crate::Error;
use common::Signer;
use elements::{
    hex::ToHex,
    pset::{
        serialize::{Deserialize, Serialize},
        PartiallySignedTransaction,
    },
    Transaction,
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

    pub fn balance(&self) -> Result<HashMap<String, u64>, Error> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Ok(m)
    }

    pub fn transaction(&self, txid: Txid) -> Result<Option<Tx>, Error> {
        Ok(self.transactions()?.into_iter().find(|e| e.txid == txid))
    }

    pub fn transactions(&self) -> Result<Vec<Tx>, Error> {
        Ok(self
            .inner
            .lock()?
            .transactions()?
            .iter()
            .map(|t| Tx {
                txid: t.tx.txid().into(),
                inputs: t
                    .inputs
                    .iter()
                    .map(|i| {
                        i.as_ref().map(|i| TxIn {
                            value: i.unblinded.value,
                            prevout_txid: i.outpoint.txid.into(),
                            prevout_vout: i.outpoint.vout,
                        })
                    })
                    .collect(),
                outputs: t
                    .outputs
                    .iter()
                    .map(|o| {
                        o.as_ref().map(|o| TxOut {
                            value: o.unblinded.value,
                            script_pubkey: o.script_pubkey.as_bytes().into(),
                        })
                    })
                    .collect(),
            })
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

    pub fn broadcast(&self, tx_hex: Hex) -> Result<Txid, Error> {
        let tx = match Transaction::deserialize(tx_hex.as_ref()) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        let wollet = self.inner.lock()?;
        match wollet.broadcast(&tx) {
            Ok(txid) => Ok(txid.into()),
            Err(e) => Err(Error::from(e)),
        }
    }
}
