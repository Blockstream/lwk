use crate::cache::Timestamp;
use crate::descriptor::Chain;
use crate::elements::bitcoin::bip32::{ChildNumber, DerivationPath};
use crate::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use crate::elements::{Address, AssetId, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::{Error, Wollet};
use lwk_common::SignedBalance;
use std::collections::HashMap;

/// Transaction details
pub struct TxDetails {
    tx: Transaction,

    // Data extracted from tx
    txid: Txid,
    fees: HashMap<AssetId, u64>,

    // Data directly extracted from store
    height: Option<u32>,
    timestamp: Option<Timestamp>,

    inputs: Vec<TxOutDetails>,
    outputs: Vec<TxOutDetails>,

    // Data derived from inputs and outputs
    type_: String,
    balance: SignedBalance,
}

impl TxDetails {
    /// Transaction
    pub fn tx(&self) -> &Transaction {
        &self.tx
    }

    /// Txid
    pub fn txid(&self) -> Txid {
        self.txid
    }

    /// Blockchain height
    pub fn height(&self) -> Option<u32> {
        self.height
    }

    /// Timestamp
    ///
    /// A reasonable timestamp, that however can be inaccurate.
    /// If you need a precise timestamp, do not use this value.
    pub fn timestamp(&self) -> Option<Timestamp> {
        self.timestamp
    }

    /// Transaction type
    ///
    /// A tentative description of the transaction type, which
    /// however might be inaccurate. Use this if you want a simple
    /// description of what this transaction is doing, but do
    /// not rely on the value returned.
    pub fn tx_type(&self) -> &str {
        &self.type_
    }

    /// Balance
    ///
    /// Net balance from the `Wollet` perspective
    pub fn balance(&self) -> &SignedBalance {
        &self.balance
    }

    /// Fees
    pub fn fees(&self) -> &HashMap<AssetId, u64> {
        &self.fees
    }

    /// Asset fees
    pub fn fees_asset(&self, asset: &AssetId) -> u64 {
        *self.fees.get(asset).unwrap_or(&0)
    }

    /// Inputs
    pub fn inputs(&self) -> &[TxOutDetails] {
        &self.inputs
    }

    /// Outputs
    pub fn outputs(&self) -> &[TxOutDetails] {
        &self.outputs
    }
}

impl Wollet {
    /// Get the details of a transaction
    pub fn tx_details(&self, txid: &Txid) -> Result<Option<TxDetails>, Error> {
        let params = self.network().address_params();
        if let Some(tx) = self.cache.all_txs.get(txid) {
            let height = self.cache.heights.get(txid).unwrap_or(&None);
            let timestamp = height.and_then(|h| self.cache.timestamps.get(&h).cloned());
            let mut outputs = vec![];
            for (vout, txout) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(*txid, vout as u32);
                let script_pubkey = txout.script_pubkey.clone();
                let (ext_int, wildcard_index, blinding_pubkey) =
                    if let Some((chain, idx)) = self.cache.paths.get(&script_pubkey) {
                        let blinding_pubkey = if let Some((_, blinding_pubkey)) =
                            self.cache.scripts.get(&(*chain, *idx))
                        {
                            *blinding_pubkey
                        } else {
                            None
                        };
                        (Some(*chain), Some(*idx), blinding_pubkey)
                    } else {
                        (None, None, None)
                    };
                let address = Address::from_script(&script_pubkey, blinding_pubkey, params);
                let unblinded = self.cache.unblinded.get(&outpoint).copied();
                outputs.push(TxOutDetails {
                    outpoint,
                    script_pubkey: Some(script_pubkey),
                    address,
                    height: *height,
                    // TODO: set this
                    is_spent: false,
                    wildcard_index,
                    ext_int,
                    unblinded,
                })
            }
            Ok(Some(TxDetails {
                tx: tx.clone(),
                txid: *txid,
                fees: tx.all_fees(),
                height: *height,
                timestamp,
                // TODO: fill these fields
                inputs: vec![],
                outputs,
                type_: "".into(),
                balance: SignedBalance::default(),
            }))
        } else {
            Ok(None)
        }
    }
}

// TODO: consider having different types for input and outputs

/// Transaction output details
pub struct TxOutDetails {
    outpoint: OutPoint,
    script_pubkey: Option<Script>,
    address: Option<Address>,

    // The height of the tx this output belong to (if available)
    height: Option<u32>,

    // TODO: this is the most expensive/annoying to compute
    #[allow(unused)]
    is_spent: bool,

    // Path
    wildcard_index: Option<ChildNumber>,
    ext_int: Option<Chain>,

    unblinded: Option<TxOutSecrets>,
}

#[allow(unused)]
impl TxOutDetails {
    /// Outpoint
    pub fn outpoint(&self) -> OutPoint {
        self.outpoint
    }

    /// Scriptpubkey
    pub fn script_pubkey(&self) -> Option<&Script> {
        self.script_pubkey.as_ref()
    }

    /// Height
    pub fn height(&self) -> Option<u32> {
        self.height
    }

    /// Path
    pub fn path(&self) -> Option<DerivationPath> {
        if let (Some(chain), Some(index)) = (self.ext_int, self.wildcard_index) {
            let chain = match chain {
                Chain::External => 0,
                Chain::Internal => 1,
            };
            let path = DerivationPath::from(vec![
                ChildNumber::from_normal_idx(chain).expect("unhardened"),
                index,
            ]);
            Some(path)
        } else {
            None
        }
    }

    /// Address
    pub fn address(&self) -> Option<&Address> {
        self.address.as_ref()
    }

    /// Whether the transaction output is explicit
    pub fn is_explicit(&self) -> bool {
        self.unblinded
            .map(|u| {
                u.asset_bf == AssetBlindingFactor::zero()
                    && u.value_bf == ValueBlindingFactor::zero()
            })
            .unwrap_or(false)
    }
}
