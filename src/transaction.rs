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
