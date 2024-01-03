use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
};

use common::Signer;
use desc::SingleSigCTDesc;
use elements::{
    hex::ToHex,
    pset::{
        serialize::{Deserialize, Serialize},
        PartiallySignedTransaction,
    },
    Transaction,
};

mod desc;
mod error;
mod network;
pub mod tx;
pub mod types;

pub use error::Error;
use network::ElementsNetwork;
use tx::{Tx, TxIn, TxOut};
use types::{Hex, Txid};

uniffi::setup_scaffolding!();

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
        descriptor: Arc<SingleSigCTDesc>,
        datadir: String,
    ) -> Result<Arc<Self>, Error> {
        let url = network.electrum_url().to_string();
        let inner = wollet::Wollet::new(
            network.into(),
            &url,
            true,
            true,
            &datadir,
            descriptor.as_str(),
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_ks_flow() {
        let datadir = "/tmp/.ks";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let singlesig_desc = SingleSigCTDesc::new(mnemonic.to_string()).unwrap();
        let wollet = Wollet::new(
            ElementsNetwork::LiquidTestnet,
            singlesig_desc.clone(),
            datadir.to_string(),
        )
        .unwrap();
        let _latest_address = wollet.address(None); // lastUnused
        let address_0 = wollet.address(Some(0)).unwrap();
        let expected_address_0 = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        assert_eq!(expected_address_0, address_0);
        let _ = wollet.sync();
        let balance = wollet.balance();
        println!("{:?}", balance);
        let txs = wollet.transactions().unwrap();
        for tx in txs {
            for output in tx.outputs {
                let script_pubkey = match output.as_ref() {
                    Some(out) => out.script_pubkey.to_string(),
                    None => "Not a spendable scriptpubkey".to_string(),
                };
                let value = match output.as_ref() {
                    Some(out) => out.value,
                    None => 0,
                };
                println!("script_pubkey: {:?}, value: {}", script_pubkey, value)
            }
        }

        let out_address = "tlq1qq0l36r57ys6nnz3xdp0eeunyuuh9dvq2fvyzj58aqaavqksenejj7plcd8mp7d9g6rxuctnj5q4cjxlu6h4tkqzv92w860z5x";
        let satoshis = 900;
        let fee_rate = 280_f32; // this seems like absolute fees
        let pset_string = wollet
            .create_lbtc_tx(out_address.to_string(), satoshis, fee_rate)
            .unwrap();
        let signed_hex = wollet.sign_tx(mnemonic.to_string(), pset_string).unwrap();
        let txid = wollet.broadcast(signed_hex.parse().unwrap()).unwrap();
        println!("BROADCASTED TX!\nTXID: {:?}", txid);
    }
}
