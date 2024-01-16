use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Store, Timestamp};
use crate::ElectrumClient;
use crate::Wollet;
use electrum_client::bitcoin::bip32::ChildNumber;
use std::collections::{HashMap, HashSet};
use std::sync::atomic;

#[derive(Default)]
pub struct DownloadTxResult {
    pub txs: Vec<(Txid, Transaction)>,
    pub unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

pub fn sync(client: &ElectrumClient, wollet: &mut Wollet) -> Result<bool, Error> {
    let update = client.scan(wollet)?;
    let result = update.is_some();
    if let Some(update) = update {
        apply_update(&mut wollet.store, update)?
    }
    Ok(result)
}

pub struct Update {
    pub new_txs: DownloadTxResult,
    pub txid_height: HashMap<Txid, Option<Height>>,
    pub timestamps: Vec<(Height, Timestamp)>,
    pub scripts: HashMap<Script, (Chain, ChildNumber)>,
}

fn apply_update(store: &mut Store, update: Update) -> Result<(), Error> {
    let Update {
        new_txs,
        mut txid_height,
        timestamps,
        scripts,
    } = update;
    store.cache.all_txs.extend(new_txs.txs);
    store.cache.unblinded.extend(new_txs.unblinds);
    let txids_unblinded: HashSet<Txid> = store.cache.unblinded.keys().map(|o| o.txid).collect();
    txid_height.retain(|txid, _| txids_unblinded.contains(txid));
    store.cache.heights.clear();
    store.cache.heights.extend(&txid_height);
    store.cache.timestamps.extend(timestamps);
    store
        .cache
        .scripts
        .extend(scripts.clone().into_iter().map(|(a, b)| (b, a)));
    store.cache.paths.extend(scripts);
    let mut last_used_internal = None;
    let mut last_used_external = None;
    for txid in txid_height.keys() {
        if let Some(tx) = store.cache.all_txs.get(txid) {
            for output in &tx.output {
                if let Some((ext_int, ChildNumber::Normal { index })) =
                    store.cache.paths.get(&output.script_pubkey)
                {
                    match ext_int {
                        Chain::External => match last_used_external {
                            None => last_used_external = Some(index),
                            Some(last) if index > last => last_used_external = Some(index),
                            _ => {}
                        },
                        Chain::Internal => match last_used_internal {
                            None => last_used_internal = Some(index),
                            Some(last) if index > last => last_used_internal = Some(index),
                            _ => {}
                        },
                    }
                }
            }
        }
    }
    if let Some(last_used_external) = last_used_external {
        store
            .cache
            .last_unused_external
            .store(last_used_external + 1, atomic::Ordering::Relaxed);
    }
    if let Some(last_used_internal) = last_used_internal {
        store
            .cache
            .last_unused_internal
            .store(last_used_internal + 1, atomic::Ordering::Relaxed);
    }
    store.flush()?;
    Ok(())
}
