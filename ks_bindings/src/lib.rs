use std::{
    collections::HashMap,
    fmt,
    str::FromStr,
    sync::{Arc, Mutex},
};

use common::Signer;
use elements::{
    hex::ToHex,
    pset::{
        serialize::{Deserialize, Serialize},
        PartiallySignedTransaction,
    },
    Transaction,
};

mod error;
mod network;
pub mod types;

pub use error::Error;
use network::ElementsNetwork;
use types::{Hex, Txid};

uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
pub struct Tx {
    txid: Txid,
    inputs: Vec<Option<TxIn>>,
    outputs: Vec<Option<TxOut>>,
}

#[derive(uniffi::Record)]
pub struct TxIn {
    prevout_txid: Txid,
    prevout_vout: u32,
    value: u64,
}

#[derive(uniffi::Record)]
pub struct TxOut {
    script_pubkey: Hex,
    value: u64,
}

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct SingleSigCTDesc {
    val: String,
}

#[uniffi::export]
impl SingleSigCTDesc {
    #[uniffi::constructor]
    pub fn new(mnemonic: String) -> Result<Arc<Self>, Error> {
        let signer = match signer::SwSigner::new(&mnemonic) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        let script_variant = common::Singlesig::Wpkh;
        let blinding_variant = common::DescriptorBlindingKey::Slip77;
        let desc_str = match common::singlesig_desc(&signer, script_variant, blinding_variant) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        Ok(Arc::new(SingleSigCTDesc { val: desc_str }))
    }
}
impl fmt::Display for SingleSigCTDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.val)
    }
}

#[derive(uniffi::Object)]
pub struct Wollet {
    inner: Mutex<wollet::Wollet>, // every exposed method must take `&self` (no &mut) so that we need to encapsulate into Mutex
}

#[uniffi::export]
impl Wollet {
    #[uniffi::constructor]
    fn new(
        network: ElementsNetwork,
        descriptor: Arc<SingleSigCTDesc>,
        datadir: String,
    ) -> Result<Arc<Self>, Error> {
        let url = network.electrum_url().to_string();
        let inner =
            wollet::Wollet::new(network.into(), &url, true, true, &datadir, &descriptor.val)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    fn descriptor(&self) -> Result<String, Error> {
        Ok(self.inner.lock()?.descriptor().to_string())
    }

    fn address(&self, index: Option<u32>) -> Result<String, Error> {
        let wollet = self.inner.lock()?;
        let address = wollet.address(index)?;
        Ok(address.address().to_string())
    }

    fn sync(&self) -> Result<(), Error> {
        let mut wollet = self.inner.lock()?;
        wollet.sync_tip()?;
        wollet.sync_txs()?;
        Ok(())
    }

    fn balance(&self) -> Result<HashMap<String, u64>, Error> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Ok(m)
    }

    fn transaction(&self, txid: Txid) -> Result<Option<Tx>, Error> {
        Ok(self.transactions()?.into_iter().find(|e| e.txid == txid))
    }

    fn transactions(&self) -> Result<Vec<Tx>, Error> {
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

    fn create_lbtc_tx(
        &self,
        out_address: String,
        satoshis: u64,
        fee_rate: f32,
    ) -> Result<String, Error> {
        let wollet = self.inner.lock()?;
        let pset = wollet.send_lbtc(satoshis, &out_address, Some(fee_rate))?;
        Ok(pset.to_string())
    }

    fn sign_tx(&self, mnemonic: String, pset_string: String) -> Result<String, Error> {
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

    fn broadcast(&self, tx_hex: Hex) -> Result<Txid, Error> {
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
