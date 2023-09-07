use crate::error::Error;
use elements::confidential::Asset;
use elements::Txid;
use elements::{TxInWitness, TxOutWitness};
use log::trace;
use std::collections::HashMap;

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
    all_unblinded: &HashMap<elements::OutPoint, elements::TxOutSecrets>,
) -> u64 {
    let outpoint = elements::OutPoint {
        txid: tx.txid(),
        vout,
    };
    all_unblinded.get(&outpoint).unwrap().value // TODO return Result<u64>?
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
    all_unblinded: &HashMap<elements::OutPoint, elements::TxOutSecrets>,
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
    all_unblinded: &HashMap<elements::OutPoint, elements::TxOutSecrets>,
) -> HashMap<elements::issuance::AssetId, i64> {
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
    all_unblinded: &HashMap<elements::OutPoint, elements::TxOutSecrets>,
) -> Option<u64> {
    txs.get(&outpoint.txid)
        .map(|tx| get_output_satoshi(&tx, outpoint.vout, &all_unblinded))
}
