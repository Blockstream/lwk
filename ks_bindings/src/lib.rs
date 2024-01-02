use std::{
    collections::HashMap,
    fmt,
    str::FromStr,
    sync::{Arc, Mutex},
};

use common::Signer;
use elements::{
    hashes::hex::FromHex,
    hex::ToHex,
    pset::{
        serialize::{Deserialize, Serialize},
        PartiallySignedTransaction,
    },
    Transaction,
};
// use wollet::elements_miniscript::descriptor;

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
pub enum ElementsNetwork {
    Liquid,
    LiquidTestnet,
    ElementsRegtest { policy_asset: String },
}
impl From<ElementsNetwork> for wollet::ElementsNetwork {
    fn from(value: ElementsNetwork) -> Self {
        match value {
            ElementsNetwork::Liquid => wollet::ElementsNetwork::Liquid,
            ElementsNetwork::LiquidTestnet => wollet::ElementsNetwork::LiquidTestnet,
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => todo!(),
        }
    }
}

#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum Error {
    #[error("{msg}")]
    Generic { msg: String },
    // TODO change the lock().unwraps with lock()? by having a variant here
}

impl From<wollet::Error> for Error {
    fn from(value: wollet::Error) -> Self {
        Error::Generic {
            msg: value.to_string(),
        }
    }
}

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

#[derive(PartialEq, Eq)]
pub struct Txid {
    val: String,
}
impl Txid {
    pub fn txid(&self) -> elements::Txid {
        elements::Txid::from_str(&self.val).expect("enforced by invariants")
    }
}
uniffi::custom_type!(Txid, String);
impl UniffiCustomTypeConverter for Txid {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        elements::Txid::from_str(&val)?;
        Ok(Txid { val })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.val
    }
}
impl From<elements::Txid> for Txid {
    fn from(value: elements::Txid) -> Self {
        Txid {
            val: value.to_string(),
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct Hex {
    val: String,
}
impl Hex {
    pub fn bytes(&self) -> Vec<u8> {
        Vec::<u8>::from_hex(&self.val).expect("enforced by invariants")
    }
}
uniffi::custom_type!(Hex, String);
impl UniffiCustomTypeConverter for Hex {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        Vec::<u8>::from_hex(&val)?;
        Ok(Hex { val })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.val
    }
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
        let url = match network {
            ElementsNetwork::Liquid => "blockstream.info:995",
            ElementsNetwork::LiquidTestnet => "blockstream.info:465",
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => todo!(),
        };
        let inner =
            wollet::Wollet::new(network.into(), url, true, true, &datadir, &descriptor.val)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    fn descriptor(&self) -> Result<String, Error> {
        Ok(self.inner.lock().unwrap().descriptor().to_string())
    }

    fn address(&self, index: Option<u32>) -> Result<String, Error> {
        let wollet = self.inner.lock().unwrap();
        let address = wollet.address(index)?;
        Ok(address.address().to_string())
    }

    fn sync(&self) -> Result<(), Error> {
        let mut wollet = self.inner.lock().unwrap();
        wollet.sync_tip()?;
        wollet.sync_txs()?;
        Ok(())
    }

    fn balance(&self) -> Result<HashMap<String, u64>, Error> {
        let m: HashMap<_, _> = self
            .inner
            .lock()
            .unwrap()
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
            .lock()
            .unwrap()
            .transactions()?
            .iter()
            .map(|t| Tx {
                txid: Txid {
                    val: t.tx.txid().to_string(),
                },
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
                            script_pubkey: Hex {
                                val: o.script_pubkey.as_bytes().to_hex(),
                            },
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
        let wollet = self.inner.lock().unwrap();
        let pset = wollet.send_lbtc(satoshis, &out_address, Some(fee_rate))?;
        Ok(pset.to_string())
    }

    fn sign_tx(&self, mnemonic: String, pset_string: String) -> Result<String, Error> {
        let wollet = self.inner.lock().unwrap();
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

    fn broadcast(&self, tx_hex: String) -> Result<Txid, Error> {
        let wollet = self.inner.lock().unwrap();
        let tx = match Transaction::deserialize(&Hex { val: tx_hex }.bytes()) {
            Ok(result) => result,
            Err(e) => return Err(Error::Generic { msg: e.to_string() }),
        };
        match wollet.broadcast(&tx) {
            Ok(txid) => Ok(Txid { val: txid.to_hex() }),
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
                    Some(out) => &out.script_pubkey.val,
                    None => "Not a spendable scriptpubkey",
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
        let txid = wollet.broadcast(signed_hex).unwrap();
        println!("BROADCASTED TX!\nTXID: {:?}", txid.val);
    }
}
