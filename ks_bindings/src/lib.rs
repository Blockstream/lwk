use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
};

use elements::{hashes::hex::FromHex, hex::ToHex};

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
enum ElementsNetwork {
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

#[derive(uniffi::Object)]
struct Wollet {
    inner: Mutex<wollet::Wollet>, // every exposed method must take `&self` (no &mut) so that we need to encapsulate into Mutex
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

#[uniffi::export]
impl Wollet {
    #[uniffi::constructor]
    fn new(
        network: ElementsNetwork,
        descriptor: String,
        datadir: String,
    ) -> Result<Arc<Self>, Error> {
        let url = match network {
            ElementsNetwork::Liquid => "blockstream.info:995",
            ElementsNetwork::LiquidTestnet => "blockstream.info:465",
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => todo!(),
        };
        let inner = wollet::Wollet::new(network.into(), url, true, true, &datadir, &descriptor)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    fn descriptor(&self) -> Result<String, Error> {
        Ok(self.inner.lock().unwrap().descriptor().to_string())
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
}
