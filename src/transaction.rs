use crate::asset::Unblinded;
use crate::error::Error;
use crate::model::Balances;
use bitcoin::hashes::hex::{FromHex, ToHex};
use elements::confidential::{Asset, Value};
use elements::Script;
use elements::Txid;
use elements::{confidential, issuance};
use elements::{TxInWitness, TxOutWitness};
use log::{info, trace};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet};
use wally::asset_surjectionproof_size;

use std::convert::TryInto;

pub const DUST_VALUE: u64 = 546;

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

fn get_output_asset(
    tx: &elements::Transaction,
    vout: u32,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<elements::issuance::AssetId> {
    let outpoint = elements::OutPoint {
        txid: tx.txid(),
        vout,
    };
    match all_unblinded.get(&outpoint) {
        Some(unblinded) => Some(unblinded.asset),
        None => None,
    }
}

fn get_output_asset_hex(
    tx: &elements::Transaction,
    vout: u32,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<String> {
    get_output_asset(tx, vout, all_unblinded).and_then(|a| Some(a.to_hex()))
}

pub fn add_output(
    tx: &mut elements::Transaction,
    address: &elements::Address,
    value: u64,
    asset_hex: String,
) -> Result<(), Error> {
    let blinding_pubkey = address.blinding_pubkey.ok_or(Error::InvalidAddress)?;
    let bytes = blinding_pubkey.serialize();
    let byte32: [u8; 32] = bytes[1..].as_ref().try_into().unwrap();
    let asset_id = issuance::AssetId::from_hex(&asset_hex)?;
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

/// return a map asset-value for the outputs needed for this transaction to be valid
pub fn needs(
    tx: &elements::Transaction,
    fee_rate: f64,
    policy_asset: elements::issuance::AssetId,
    all_txs: &HashMap<Txid, elements::Transaction>,
    unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Vec<(elements::issuance::AssetId, u64)> {
    let mut outputs: HashMap<elements::issuance::AssetId, u64> = HashMap::new();
    for output in tx.output.iter() {
        match (output.asset, output.value) {
            (Asset::Explicit(asset), Value::Explicit(value)) => {
                *outputs.entry(asset).or_insert(0) += value;
            }
            _ => panic!("asset and value should be explicit here"),
        }
    }

    let mut inputs: HashMap<elements::issuance::AssetId, u64> = HashMap::new();

    for input in tx.input.iter() {
        let asset = get_previous_output_asset(&all_txs, input.previous_output, unblinded).unwrap();
        let value = get_previous_output_value(&all_txs, &input.previous_output, unblinded).unwrap();
        *inputs.entry(asset).or_insert(0) += value;
    }

    let estimated_fee = estimated_fee(&tx, fee_rate, estimated_changes(&tx, all_txs, unblinded));
    *outputs.entry(policy_asset).or_insert(0) += estimated_fee;

    let mut result = vec![];
    for (asset, value) in outputs.iter() {
        if let Some(sum) = value.checked_sub(inputs.remove(asset).unwrap_or(0)) {
            if sum > 0 {
                result.push((*asset, sum));
            }
        }
    }

    result
}

pub fn estimated_changes(
    tx: &elements::Transaction,
    all_txs: &HashMap<Txid, elements::Transaction>,
    unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> u8 {
    let mut different_assets = HashSet::new();
    for input in tx.input.iter() {
        let asset_hex =
            get_previous_output_asset_hex(&all_txs, input.previous_output, unblinded).unwrap();
        different_assets.insert(asset_hex.clone());
    }
    if different_assets.is_empty() {
        0
    } else {
        different_assets.len() as u8
    }
}

/// return a map asset-value for the changes of this transaction
/// requires inputs are greater than outputs for earch asset
pub fn changes(
    tx: &elements::Transaction,
    estimated_fee: u64,
    policy_asset: elements::issuance::AssetId,
    all_txs: &HashMap<Txid, elements::Transaction>,
    unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> HashMap<elements::issuance::AssetId, u64> {
    let mut outputs_asset_amounts: HashMap<elements::issuance::AssetId, u64> = HashMap::new();
    for output in tx.output.iter() {
        match (output.asset, output.value) {
            (Asset::Explicit(asset), Value::Explicit(value)) => {
                *outputs_asset_amounts.entry(asset).or_insert(0) += value;
            }
            _ => panic!("asset and value should be explicit here"),
        }
    }

    let mut inputs_asset_amounts: HashMap<elements::issuance::AssetId, u64> = HashMap::new();
    for input in tx.input.iter() {
        let asset = get_previous_output_asset(&all_txs, input.previous_output, unblinded).unwrap();
        let value = get_previous_output_value(&all_txs, &input.previous_output, unblinded).unwrap();
        *inputs_asset_amounts.entry(asset).or_insert(0) += value;
    }
    let mut result: HashMap<elements::issuance::AssetId, u64> = HashMap::new();
    for (asset, value) in inputs_asset_amounts.iter() {
        let mut sum: u64 = value - outputs_asset_amounts.remove(asset).unwrap_or(0);
        if *asset == policy_asset {
            // from a purely privacy perspective could make sense to always create the change output in liquid, so min change = 0
            // however elements core use the dust anyway for 2 reasons: rebasing from core and economical considerations
            sum -= estimated_fee;
            if sum > DUST_VALUE {
                // we apply dust rules for liquid bitcoin as elements do
                result.insert(*asset, sum);
            }
        } else if sum > 0 {
            result.insert(*asset, sum);
        }
    }
    assert!(outputs_asset_amounts.is_empty());
    result
}

pub fn add_fee_output(
    tx: &mut elements::Transaction,
    value: u64,
    policy_asset: &Option<Asset>,
) -> Result<(), Error> {
    let policy_asset = policy_asset.ok_or_else(|| Error::Generic("Missing policy asset".into()))?;
    let new_out = elements::TxOut {
        asset: policy_asset,
        value: confidential::Value::Explicit(value),
        ..Default::default()
    };
    tx.output.push(new_out);
    Ok(())
}

pub fn add_input(tx: &mut elements::Transaction, outpoint: elements::OutPoint) {
    let new_in = elements::TxIn {
        previous_output: outpoint,
        is_pegin: false,
        has_issuance: false,
        script_sig: Script::default(),
        sequence: 0xffff_fffe, // nSequence is disabled, nLocktime is enabled, RBF is not signaled.
        asset_issuance: Default::default(),
        witness: TxInWitness::default(),
    };
    tx.input.push(new_in);
}

/// calculate transaction fee,
/// for bitcoin it requires all previous output to get input values.
/// for elements,
///     for complete transactions looks at the explicit fee output,
///     for incomplete tx (without explicit fee output) take the sum previous outputs value, previously unblinded
///                       and use the outputs value that must be still unblinded
pub fn fee(
    tx: &elements::Transaction,
    all_txs: &HashMap<Txid, elements::Transaction>,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
    policy_asset: &Option<Asset>,
) -> Result<u64, Error> {
    Ok({
        let has_fee = tx.output.iter().any(|o| o.is_fee());

        if has_fee {
            let policy_asset =
                policy_asset.ok_or_else(|| Error::Generic("Missing policy asset".into()))?;
            tx.output
                .iter()
                .filter(|o| o.is_fee())
                .filter(|o| policy_asset == o.asset)
                .map(|o| o.minimum_value()) // minimum_value used for extracting the explicit value (value is always explicit for fee)
                .sum::<u64>()
        } else {
            // while we are not filtering assets, the following holds for valid tx because
            // sum of input assets = sum of output assets
            let sum_outputs: u64 = tx.output.iter().map(|o| o.minimum_value()).sum();
            let sum_inputs: u64 = tx
                .input
                .iter()
                .map(|i| i.previous_output)
                .filter_map(|o| get_previous_output_value(&all_txs, &o, all_unblinded))
                .sum();

            sum_inputs - sum_outputs
        }
    })
}

pub fn my_balance_changes(
    tx: &elements::Transaction,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Balances {
    trace!(
        "tx_id: {} my_balances elements all_unblinded.len(): {:?}",
        tx.txid(),
        all_unblinded
    );
    let mut result = HashMap::new();
    for input in tx.input.iter() {
        let outpoint = input.previous_output;
        if let Some(unblinded) = all_unblinded.get(&outpoint) {
            trace!(
                "tx_id: {} unblinded previous output {} {}",
                tx.txid(),
                outpoint,
                unblinded.value
            );
            *result.entry(unblinded.asset).or_default() -= unblinded.value as i64;
            // TODO check overflow
        }
    }
    for i in 0..tx.output.len() as u32 {
        let outpoint = elements::OutPoint {
            txid: tx.txid(),
            vout: i,
        };
        if let Some(unblinded) = all_unblinded.get(&outpoint) {
            trace!(
                "tx_id: {} unblinded output {} {}",
                tx.txid(),
                outpoint,
                unblinded.value
            );
            *result.entry(unblinded.asset).or_default() += unblinded.value as i64;
            // TODO check overflow
        }
    }

    // we don't want to see redeposited assets
    return result.into_iter().filter(|&(_, v)| v != 0).collect();
}

pub fn get_previous_output_value(
    txs: &HashMap<Txid, elements::Transaction>,
    outpoint: &elements::OutPoint,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<u64> {
    txs.get(&outpoint.txid)
        .map(|tx| get_output_satoshi(&tx, outpoint.vout, &all_unblinded))
}

pub fn get_previous_output_asset(
    txs: &HashMap<Txid, elements::Transaction>,
    outpoint: elements::OutPoint,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<elements::issuance::AssetId> {
    txs.get(&outpoint.txid)
        .map(|tx| get_output_asset(&tx, outpoint.vout, &all_unblinded).unwrap())
}

pub fn get_previous_output_asset_hex(
    txs: &HashMap<Txid, elements::Transaction>,
    outpoint: elements::OutPoint,
    all_unblinded: &HashMap<elements::OutPoint, Unblinded>,
) -> Option<String> {
    get_previous_output_asset(txs, outpoint, all_unblinded).and_then(|a| Some(a.to_hex()));
    txs.get(&outpoint.txid)
        .map(|tx| get_output_asset_hex(&tx, outpoint.vout, &all_unblinded).unwrap())
}
