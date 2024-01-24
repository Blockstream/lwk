use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Timestamp};
use crate::Wollet;
use electrum_client::bitcoin::bip32::ChildNumber;
use elements::BlockHeader;
use std::collections::{HashMap, HashSet};
use std::sync::atomic;

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct DownloadTxResult {
    pub txs: Vec<(Txid, Transaction)>,
    pub unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

/// Passing a wallet to [`crate::BlockchainBackend::full_scan()`] returns this structure which
/// contains the delta of information to be applied to the wallet to reach the latest status.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Update {
    pub new_txs: DownloadTxResult,
    pub txid_height_new: Vec<(Txid, Option<Height>)>,
    pub txid_height_delete: Vec<Txid>,
    pub timestamps: Vec<(Height, Timestamp)>,
    pub scripts: HashMap<Script, (Chain, ChildNumber)>,
    pub tip: BlockHeader,
}

impl Wollet {
    pub fn apply_update(&mut self, update: Update) -> Result<(), Error> {
        // TODO should accept &Update
        let store = &mut self.store;
        let Update {
            new_txs,
            txid_height_new,
            txid_height_delete,
            timestamps,
            scripts,
            tip,
        } = update;
        store.cache.tip = (tip.height, tip.block_hash());
        store.cache.all_txs.extend(new_txs.txs);
        store.cache.unblinded.extend(new_txs.unblinds);
        let txids_unblinded: HashSet<Txid> = store.cache.unblinded.keys().map(|o| o.txid).collect();
        let txid_height: Vec<_> = txid_height_new
            .iter()
            .filter(|(txid, _)| txids_unblinded.contains(txid))
            .cloned()
            .collect();
        store
            .cache
            .heights
            .retain(|k, _| !txid_height_delete.contains(k));
        store.cache.heights.extend(txid_height.clone());
        store.cache.timestamps.extend(timestamps);
        store
            .cache
            .scripts
            .extend(scripts.clone().into_iter().map(|(a, b)| (b, a)));
        store.cache.paths.extend(scripts);
        let mut last_used_internal = None;
        let mut last_used_external = None;
        for (txid, _) in txid_height {
            if let Some(tx) = store.cache.all_txs.get(&txid) {
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
}
