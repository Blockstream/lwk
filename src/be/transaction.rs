use crate::be::*;
use crate::error::Error;
use crate::model::Balances;
use bitcoin::hash_types::Txid;
use bitcoin::Script;
use elements::confidential::{Asset, Value};
use elements::encode::deserialize as elm_des;
use elements::encode::serialize as elm_ser;
use elements::{confidential, issuance};
use elements::{TxInWitness, TxOutWitness};
use log::{info, trace};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use wally::asset_surjectionproof_size;

pub const DUST_VALUE: u64 = 546;

#[derive(Default, Debug, Clone, Serialize, Deserialize, Hash)]
pub struct ETransaction(pub elements::Transaction);

impl Deref for ETransaction {
    type Target = elements::Transaction;
    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl DerefMut for ETransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn strip_witness(tx: &mut elements::Transaction) {
    for input in tx.input.iter_mut() {
        input.witness = TxInWitness::default();
    }
    for output in tx.output.iter_mut() {
        output.witness = TxOutWitness::default();
    }
}

fn get_output_satoshi(
    tx: &elements::Transaction,
    vout: u32,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> u64 {
    let outpoint = elements::OutPoint {
        txid: tx.txid(),
        vout,
    };
    all_unblinded.get(&outpoint).unwrap().value // TODO return Result<u64>?
}

fn get_output_asset_hex(
    tx: &elements::Transaction,
    vout: u32,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<String> {
    let outpoint = elements::OutPoint {
        txid: tx.txid(),
        vout,
    };
    match all_unblinded.get(&outpoint) {
        Some(unblinded) => Some(unblinded.asset_hex()),
        None => None,
    }
}

pub fn add_output(
    tx: &mut elements::Transaction,
    address: &str,
    value: u64,
    asset_hex: String,
) -> Result<(), Error> {
    let address = elements::Address::from_str(&address).map_err(|_| Error::InvalidAddress)?;
    let blinding_pubkey = address.blinding_pubkey.ok_or(Error::InvalidAddress)?;
    let bytes = blinding_pubkey.serialize();
    let byte32: [u8; 32] = bytes[1..].as_ref().try_into().unwrap();
    let asset = asset_to_bin(&asset_hex).expect("invalid asset hex");
    let asset_id = issuance::AssetId::from_slice(&asset)?;
    let new_out = elements::TxOut {
        asset: confidential::Asset::Explicit(asset_id),
        value: confidential::Value::Explicit(value),
        nonce: confidential::Nonce::Confidential(bytes[0], byte32),
        script_pubkey: address.script_pubkey(),
        witness: TxOutWitness::default(),
    };
    tx.output.push(new_out);
    Ok(())
}

pub fn scramble(tx: &mut elements::Transaction) {
    let mut rng = thread_rng();
    tx.input.shuffle(&mut rng);
    tx.output.shuffle(&mut rng);
}

/// estimates the fee of the final transaction given the `fee_rate`
/// called when the tx is being built and miss things like signatures and changes outputs.
pub fn estimated_fee(tx: &elements::Transaction, fee_rate: f64, more_changes: u8) -> u64 {
    let mut tx = tx.clone();
    for input in tx.input.iter_mut() {
        let mut tx_wit = TxInWitness::default();
        tx_wit.script_witness = vec![vec![0u8; 72], vec![0u8; 33]]; // considering signature sizes (72) and compressed public key (33)
        input.witness = tx_wit;
        input.script_sig = vec![0u8; 23].into(); // p2shwpkh redeem script size
    }
    for _ in 0..more_changes {
        let new_out = elements::TxOut {
            asset: confidential::Asset::Confidential(0u8, [0u8; 32]),
            value: confidential::Value::Confidential(0u8, [0u8; 32]),
            nonce: confidential::Nonce::Confidential(0u8, [0u8; 32]),
            ..Default::default()
        };
        tx.output.push(new_out);
    }
    let sur_size = asset_surjectionproof_size(std::cmp::max(1, tx.input.len()));
    for output in tx.output.iter_mut() {
        output.witness = TxOutWitness {
            surjection_proof: vec![0u8; sur_size],
            rangeproof: vec![0u8; 4174],
        };
        output.script_pubkey = vec![0u8; 21].into();
    }

    tx.output.push(elements::TxOut::default()); // mockup for the explicit fee output
    let vbytes = tx.get_weight() as f64 / 4.0;
    let fee_val = (vbytes * fee_rate * 1.03) as u64; // increasing estimated fee by 3% to stay over relay fee, TODO improve fee estimation and lower this
    info!(
        "DUMMYTX inputs:{} outputs:{} num_changes:{} vbytes:{} sur_size:{} fee_val:{}",
        tx.input.len(),
        tx.output.len(),
        more_changes,
        vbytes,
        sur_size,
        fee_val
    );
    fee_val
}

impl ETransaction {
    pub fn new() -> Self {
        ETransaction(elements::Transaction {
            version: 2,
            lock_time: 0,
            input: vec![],
            output: vec![],
        })
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, crate::error::Error> {
        Ok(ETransaction(elm_des(bytes)?))
    }

    pub fn from_hex(hex: &str) -> Result<Self, crate::error::Error> {
        Self::deserialize(&hex::decode(hex)?)
    }

    pub fn serialize(&self) -> Vec<u8> {
        elm_ser(&self.0)
    }

    pub fn txid(&self) -> Txid {
        self.0.txid()
    }

    pub fn get_weight(&self) -> usize {
        self.0.get_weight()
    }

    pub fn estimated_changes(
        &self,
        send_all: bool,
        all_txs: &ETransactions,
        unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> u8 {
        let mut different_assets = HashSet::new();
        for input in self.0.input.iter() {
            let asset_hex = all_txs
                .get_previous_output_asset_hex(input.previous_output, unblinded)
                .unwrap();
            different_assets.insert(asset_hex.clone());
        }
        if different_assets.is_empty() {
            0
        } else {
            different_assets.len() as u8 - send_all as u8
        }
    }

    /// return a Vector with the amount needed for this transaction to be valid
    /// for bitcoin it contains max 1 element eg ("btc", 100)
    /// for elements could contain more than 1 element, 1 for each asset, with the policy asset last
    pub fn needs(
        &self,
        fee_rate: f64,
        no_change: bool,
        policy_asset: Option<String>,
        all_txs: &ETransactions,
        unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> Vec<AssetValue> {
        let policy_asset = policy_asset.expect("policy asset empty in elements");
        let mut outputs: HashMap<String, u64> = HashMap::new();
        for output in self.0.output.iter() {
            match (output.asset, output.value) {
                (Asset::Explicit(asset), Value::Explicit(value)) => {
                    let asset_hex = asset_to_hex(&asset.into_inner());
                    *outputs.entry(asset_hex).or_insert(0) += value;
                }
                _ => panic!("asset and value should be explicit here"),
            }
        }

        let mut inputs: HashMap<String, u64> = HashMap::new();

        for input in self.0.input.iter() {
            let asset_hex = all_txs
                .get_previous_output_asset_hex(input.previous_output, unblinded)
                .unwrap();
            let value = all_txs
                .get_previous_output_value(&input.previous_output, unblinded)
                .unwrap();
            *inputs.entry(asset_hex).or_insert(0) += value;
        }

        let estimated_fee = estimated_fee(
            &self.0,
            fee_rate,
            self.estimated_changes(no_change, all_txs, unblinded),
        );
        *outputs.entry(policy_asset.clone()).or_insert(0) += estimated_fee;

        let mut result = vec![];
        for (asset, value) in outputs.iter() {
            if let Some(sum) = value.checked_sub(inputs.remove(asset).unwrap_or(0)) {
                if sum > 0 {
                    result.push(AssetValue::new(asset.to_string(), sum));
                }
            }
        }

        if let Some(index) = result.iter().position(|e| e.asset == policy_asset) {
            let last_index = result.len() - 1;
            if index != last_index {
                result.swap(index, last_index); // put the policy asset last
            }
        }
        result
    }

    /// return a Vector with changes of this transaction
    /// requires inputs are greater than outputs for earch asset
    pub fn changes(
        &self,
        estimated_fee: u64,
        policy_asset: Option<String>,
        all_txs: &ETransactions,
        unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> Vec<AssetValue> {
        let mut outputs_asset_amounts: HashMap<String, u64> = HashMap::new();
        for output in self.0.output.iter() {
            match (output.asset, output.value) {
                (Asset::Explicit(asset), Value::Explicit(value)) => {
                    let asset_hex = asset_to_hex(&asset.into_inner());
                    *outputs_asset_amounts.entry(asset_hex).or_insert(0) += value;
                }
                _ => panic!("asset and value should be explicit here"),
            }
        }

        let mut inputs_asset_amounts: HashMap<String, u64> = HashMap::new();
        for input in self.0.input.iter() {
            let asset_hex = all_txs
                .get_previous_output_asset_hex(input.previous_output, unblinded)
                .unwrap();
            let value = all_txs
                .get_previous_output_value(&input.previous_output, unblinded)
                .unwrap();
            *inputs_asset_amounts.entry(asset_hex).or_insert(0) += value;
        }
        let mut result = vec![];
        for (asset, value) in inputs_asset_amounts.iter() {
            let mut sum = value - outputs_asset_amounts.remove(asset).unwrap_or(0);
            if asset == policy_asset.as_ref().unwrap() {
                // from a purely privacy perspective could make sense to always create the change output in liquid, so min change = 0
                // however elements core use the dust anyway for 2 reasons: rebasing from core and economical considerations
                sum -= estimated_fee;
                if sum > DUST_VALUE {
                    // we apply dust rules for liquid bitcoin as elements do
                    result.push(AssetValue::new(asset.to_string(), sum));
                }
            } else if sum > 0 {
                result.push(AssetValue::new(asset.to_string(), sum));
            }
        }
        assert!(outputs_asset_amounts.is_empty());
        result
    }

    pub fn add_fee_if_elements(
        &mut self,
        value: u64,
        policy_asset: &Option<Asset>,
    ) -> Result<(), Error> {
        let policy_asset =
            policy_asset.ok_or_else(|| Error::Generic("Missing policy asset".into()))?;
        let new_out = elements::TxOut {
            asset: policy_asset,
            value: confidential::Value::Explicit(value),
            ..Default::default()
        };
        self.0.output.push(new_out);
        Ok(())
    }

    pub fn add_input(&mut self, outpoint: elements::OutPoint) {
        let new_in = elements::TxIn {
            previous_output: outpoint,
            is_pegin: false,
            has_issuance: false,
            script_sig: Script::default(),
            sequence: 0xffff_fffe, // nSequence is disabled, nLocktime is enabled, RBF is not signaled.
            asset_issuance: Default::default(),
            witness: TxInWitness::default(),
        };
        self.0.input.push(new_in);
    }

    /// calculate transaction fee,
    /// for bitcoin it requires all previous output to get input values.
    /// for elements,
    ///     for complete transactions looks at the explicit fee output,
    ///     for incomplete tx (without explicit fee output) take the sum previous outputs value, previously unblinded
    ///                       and use the outputs value that must be still unblinded
    pub fn fee(
        &self,
        all_txs: &ETransactions,
        all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
        policy_asset: &Option<Asset>,
    ) -> Result<u64, Error> {
        Ok({
            let has_fee = self.0.output.iter().any(|o| o.is_fee());

            if has_fee {
                let policy_asset =
                    policy_asset.ok_or_else(|| Error::Generic("Missing policy asset".into()))?;
                self.0
                    .output
                    .iter()
                    .filter(|o| o.is_fee())
                    .filter(|o| policy_asset == o.asset)
                    .map(|o| o.minimum_value()) // minimum_value used for extracting the explicit value (value is always explicit for fee)
                    .sum::<u64>()
            } else {
                // while we are not filtering assets, the following holds for valid tx because
                // sum of input assets = sum of output assets
                let sum_outputs: u64 = self.0.output.iter().map(|o| o.minimum_value()).sum();
                let sum_inputs: u64 = self
                    .0
                    .input
                    .iter()
                    .map(|i| i.previous_output)
                    .filter_map(|o| all_txs.get_previous_output_value(&o, all_unblinded))
                    .sum();

                sum_inputs - sum_outputs
            }
        })
    }

    pub fn my_balance_changes(
        &self,
        all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> Balances {
        trace!(
            "tx_id: {} my_balances elements all_unblinded.len(): {:?}",
            self.0.txid(),
            all_unblinded
        );
        let mut result = HashMap::new();
        for input in self.0.input.iter() {
            let outpoint = input.previous_output;
            if let Some(unblinded) = all_unblinded.get(&outpoint) {
                trace!(
                    "tx_id: {} unblinded previous output {} {}",
                    self.0.txid(),
                    outpoint,
                    unblinded.value
                );
                let asset_id_str = unblinded.asset_hex();
                *result.entry(asset_id_str).or_default() -= unblinded.value as i64;
                // TODO check overflow
            }
        }
        for i in 0..self.0.output.len() as u32 {
            let outpoint = elements::OutPoint {
                txid: self.0.txid(),
                vout: i,
            };
            if let Some(unblinded) = all_unblinded.get(&outpoint) {
                trace!(
                    "tx_id: {} unblinded output {} {}",
                    self.0.txid(),
                    outpoint,
                    unblinded.value
                );
                let asset_id_str = unblinded.asset_hex();
                *result.entry(asset_id_str).or_default() += unblinded.value as i64;
                // TODO check overflow
            }
        }

        // we don't want to see redeposited assets
        return result.into_iter().filter(|&(_, v)| v != 0).collect();
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ETransactions(HashMap<Txid, ETransaction>);

impl Deref for ETransactions {
    type Target = HashMap<Txid, ETransaction>;
    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}
impl DerefMut for ETransactions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl ETransactions {
    pub fn get_previous_output_script_pubkey(
        &self,
        outpoint: &elements::OutPoint,
    ) -> Option<Script> {
        self.0
            .get(&outpoint.txid)
            .map(|tx| tx.output[outpoint.vout as usize].script_pubkey.clone())
    }
    pub fn get_previous_output_value(
        &self,
        outpoint: &elements::OutPoint,
        all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> Option<u64> {
        self.0
            .get(&outpoint.txid)
            .map(|tx| get_output_satoshi(&tx.0, outpoint.vout, &all_unblinded))
    }

    pub fn get_previous_output_asset_hex(
        &self,
        outpoint: elements::OutPoint,
        all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
    ) -> Option<String> {
        self.0
            .get(&outpoint.txid)
            .map(|tx| get_output_asset_hex(&tx, outpoint.vout, &all_unblinded).unwrap())
    }
}

//TODO remove this, `fn needs` could return BTreeMap<String, u64> instead
#[derive(Debug)]
pub struct AssetValue {
    pub asset: String,
    pub satoshi: u64,
}

impl AssetValue {
    fn new(asset: String, satoshi: u64) -> Self {
        AssetValue { asset, satoshi }
    }
}
