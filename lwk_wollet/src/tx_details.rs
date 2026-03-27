use crate::cache::Timestamp;
use crate::descriptor::Chain;
use crate::elements::bitcoin::bip32::{ChildNumber, DerivationPath};
use crate::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use crate::elements::secp256k1_zkp::ZERO_TWEAK;
use crate::elements::{Address, AssetId, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::{Error, Wollet};
use lwk_common::SignedBalance;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Transaction details
#[derive(PartialEq, Eq, Debug, Clone)]
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
    pub fn tx(&self) -> Option<&Transaction> {
        Some(&self.tx)
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
    fn txout_details(
        &self,
        outpoint: OutPoint,
        height: Option<u32>,
        txout: Option<&elements::TxOut>,
        is_spent: bool,
    ) -> Result<TxOutDetails, Error> {
        let mut details = TxOutDetails {
            outpoint,
            height,
            is_spent,
            ..Default::default()
        };
        if let Some(txout) = txout {
            let script_pubkey = txout.script_pubkey.clone();
            let (ext_int, wildcard_index, blinding_pubkey) = if let Some((chain, idx)) =
                self.cache.paths.get(&script_pubkey)
            {
                let blinding_pubkey =
                    if let Some((_, blinding_pubkey)) = self.cache.scripts.get(&(*chain, *idx)) {
                        *blinding_pubkey
                    } else {
                        None
                    };
                (Some(*chain), Some(*idx), blinding_pubkey)
            } else {
                // Output does not belong to the wallet, we can't say if it's spent, default to false
                details.is_spent = false;
                (None, None, None)
            };
            details.ext_int = ext_int;
            details.wildcard_index = wildcard_index;
            let params = self.network().address_params();
            details.address = Address::from_script(&script_pubkey, blinding_pubkey, params);
            details.script_pubkey = Some(script_pubkey);
            details.unblinded = if let (Some(value), Some(asset)) =
                (txout.value.explicit(), txout.asset.explicit())
            {
                Some(TxOutSecrets::new(
                    asset,
                    AssetBlindingFactor::zero(),
                    value,
                    ValueBlindingFactor::zero(),
                ))
            } else {
                self.cache.unblinded.get(&outpoint).copied()
            };
        }
        Ok(details)
    }

    fn tx_details_inner(
        &self,
        txid: &Txid,
        height: Option<u32>,
        unspent: &HashSet<OutPoint>,
    ) -> Result<Option<TxDetails>, Error> {
        if let Some(tx) = self.cache.tx(txid) {
            let timestamp = height.and_then(|h| self.cache.timestamps.get(&h).cloned());
            let mut inputs = vec![];
            for txin in &tx.input {
                let outpoint = txin.previous_output;
                let height = self.cache.tx_height(&outpoint.txid).unwrap_or(&None);
                let txout = self
                    .cache
                    .tx(&outpoint.txid)
                    .and_then(|tx| tx.output.get(outpoint.vout as usize));
                let is_spent = true; // inputs are always spent
                inputs.push(self.txout_details(outpoint, *height, txout, is_spent)?);
            }
            let mut outputs = vec![];
            for (vout, txout) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(*txid, vout as u32);
                let is_spent = !unspent.contains(&outpoint);
                outputs.push(self.txout_details(outpoint, height, Some(txout), is_spent)?);
            }

            let balance: SignedBalance = {
                let mut b = BTreeMap::new();
                // For net balance computation we ignore explicit inputs and outputs
                for i in &inputs {
                    if i.path().is_some() && !i.is_explicit() {
                        if let Some(u) = i.unblinded() {
                            *b.entry(u.asset).or_default() -= u.value as i64;
                        }
                    }
                }
                for o in &outputs {
                    if o.path().is_some() && !o.is_explicit() {
                        if let Some(u) = o.unblinded() {
                            *b.entry(u.asset).or_default() += u.value as i64;
                        }
                    }
                }
                b.into()
            };

            let fees = tx.all_fees();
            let type_ = {
                let burn_script = lwk_common::burn_script();
                if tx.input.iter().any(|i| {
                    !i.asset_issuance.is_null()
                        && i.asset_issuance.asset_blinding_nonce == ZERO_TWEAK
                }) {
                    "issuance".to_string()
                } else if tx.input.iter().any(|i| {
                    !i.asset_issuance.is_null()
                        && i.asset_issuance.asset_blinding_nonce != ZERO_TWEAK
                }) {
                    "reissuance".to_string()
                } else if tx.output.iter().any(|o| o.script_pubkey == burn_script) {
                    "burn".to_string()
                } else if !fees.is_empty()
                    && balance.len() == fees.len()
                    && fees
                        .iter()
                        .all(|(asset, fee)| balance.get(asset) == Some(&-(*fee as i64)))
                {
                    "redeposit".to_string()
                } else if balance.is_empty() {
                    "unknown".to_string()
                } else if balance.values().all(|v| *v > 0) {
                    "incoming".to_string()
                } else if balance.values().all(|v| *v < 0) {
                    // redeposit case handled above
                    "outgoing".to_string()
                } else {
                    "unknown".to_string()
                }
            };

            Ok(Some(TxDetails {
                tx: tx.clone(),
                txid: *txid,
                fees,
                height,
                timestamp,
                inputs,
                outputs,
                type_,
                balance,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the details of a transaction
    ///
    /// **Unstable**: This API may change without notice.
    #[doc(hidden)]
    pub fn tx_details(&self, txid: &Txid, _opt: &TxOpt) -> Result<Option<TxDetails>, Error> {
        let unspent = self.cache.unspent();
        let height = *self.cache.tx_height(txid).unwrap_or(&None);
        self.tx_details_inner(txid, height, unspent)
    }

    /// Get the transaction list
    ///
    /// **Unstable**: This API may change without notice.
    #[doc(hidden)]
    pub fn txs(&self, opt: &TxsOpt) -> Result<Vec<TxDetails>, Error> {
        let unspent = self.cache.unspent();
        let mut txs = vec![];
        let offset = opt.offset.unwrap_or(0);
        let limit = opt.limit.unwrap_or(usize::MAX);
        for (txid, height) in self.cache.sorted_txids().skip(offset).take(limit) {
            if let Some(tx) = self.tx_details_inner(txid, *height, unspent)? {
                txs.push(tx);
            }
        }
        Ok(txs)
    }
}

/// Options for transaction details
#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct TxOpt {}

// TODO: consider removing options and set deafult values here
/// Options for transaction details
#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct TxsOpt {
    /// Do not return the first `offset` transactions
    pub offset: Option<usize>,
    /// Return at most `limit` transactions
    pub limit: Option<usize>,
}

// TODO: consider having different types for input and outputs

/// Transaction output details
#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct TxOutDetails {
    outpoint: OutPoint,
    script_pubkey: Option<Script>,
    address: Option<Address>,

    // The height of the tx this output belong to (if available)
    height: Option<u32>,

    is_spent: bool,

    // Path
    wildcard_index: Option<ChildNumber>,
    ext_int: Option<Chain>,

    unblinded: Option<TxOutSecrets>,
}

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

    /// Unblinded values (asset, amount, blinders)
    pub fn unblinded(&self) -> Option<TxOutSecrets> {
        self.unblinded
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

    /// Whether the output is spent by a previously downloaded transaction
    ///
    /// Note: this value might be inaccurate. We compute this from downloaded
    /// transactions, however we only download transactions relevant for the
    /// wallet (i.e. if they include inputs or outputs that belong to the
    /// wallet), thus for non-wallet outputs we might set this value
    /// incorrectly. For wallet outputs, it can be outdated.
    pub fn is_spent(&self) -> bool {
        self.is_spent
    }
}
