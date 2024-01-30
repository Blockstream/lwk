use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Timestamp};
use crate::Wollet;
use electrum_client::bitcoin::bip32::ChildNumber;
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::{Decodable, Encodable};
use elements::BlockHeader;
use std::collections::{HashMap, HashSet};
use std::sync::atomic;

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct DownloadTxResult {
    pub txs: Vec<(Txid, Transaction)>,
    pub unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

impl DownloadTxResult {
    fn is_empty(&self) -> bool {
        self.txs.is_empty() && self.unblinds.is_empty()
    }
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

impl Update {
    pub fn only_tip(&self) -> bool {
        self.new_txs.is_empty()
            && self.txid_height_new.is_empty()
            && self.txid_height_delete.is_empty()
            && self.timestamps.is_empty()
            && self.scripts.is_empty()
    }
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

impl Encodable for DownloadTxResult {
    fn consensus_encode<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> Result<usize, elements::encode::Error> {
        let mut bytes_written = 0;

        let txs_len = self.txs.len();
        bytes_written += elements::VarInt(txs_len as u64).consensus_encode(&mut w)?;
        for (_txid, tx) in self.txs.iter() {
            // Avoid serializing Txid since are re-computable from the tx
            bytes_written += tx.consensus_encode(&mut w)?;
        }

        let unblinds_len = self.unblinds.len();
        bytes_written += elements::VarInt(unblinds_len as u64).consensus_encode(&mut w)?;
        for (out_point, tx_out_secrets) in self.unblinds.iter() {
            bytes_written += out_point.consensus_encode(&mut w)?;

            // TODO make TxOutSecrets encodable upstream
            let encodable_tx_out_secrets = EncodableTxOutSecrets {
                inner: tx_out_secrets.clone(),
            };
            bytes_written += encodable_tx_out_secrets.consensus_encode(&mut w)?;
        }

        Ok(bytes_written)
    }
}

impl Decodable for DownloadTxResult {
    fn consensus_decode<D: std::io::Read>(mut d: D) -> Result<Self, elements::encode::Error> {
        let mut txs = vec![];
        let txs_len = elements::VarInt::consensus_decode(&mut d)?.0;
        for _ in 0..txs_len {
            let tx = Transaction::consensus_decode(&mut d)?;
            txs.push((tx.txid(), tx));
        }

        let mut unblinds = vec![];
        let unblinds_len = elements::VarInt::consensus_decode(&mut d)?.0;
        for _ in 0..unblinds_len {
            let out_point = OutPoint::consensus_decode(&mut d)?;
            let encodable_tx_out_secrets = EncodableTxOutSecrets::consensus_decode(&mut d)?;
            unblinds.push((out_point, encodable_tx_out_secrets.inner))
        }

        Ok(DownloadTxResult { txs, unblinds })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct EncodableTxOutSecrets {
    inner: TxOutSecrets,
}
impl Encodable for EncodableTxOutSecrets {
    fn consensus_encode<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> Result<usize, elements::encode::Error> {
        let mut bytes_written = 0;
        bytes_written += self.inner.asset.consensus_encode(&mut w)?;

        bytes_written += self
            .inner
            .asset_bf
            .clone()
            .into_inner()
            .as_ref()
            .consensus_encode(&mut w)?;

        bytes_written += self.inner.value.consensus_encode(&mut w)?;

        bytes_written += self
            .inner
            .value_bf
            .clone()
            .into_inner()
            .as_ref()
            .consensus_encode(&mut w)?;

        Ok(bytes_written)
    }
}

impl Decodable for EncodableTxOutSecrets {
    fn consensus_decode<D: std::io::Read>(mut d: D) -> Result<Self, elements::encode::Error> {
        Ok(Self {
            inner: TxOutSecrets {
                asset: Decodable::consensus_decode(&mut d)?,
                asset_bf: {
                    let bytes: [u8; 32] = Decodable::consensus_decode(&mut d)?;
                    AssetBlindingFactor::from_slice(&bytes[..]).expect("bytes length is 32")
                },
                value: Decodable::consensus_decode(&mut d)?,
                value_bf: {
                    let bytes: [u8; 32] = Decodable::consensus_decode(&mut d)?;
                    ValueBlindingFactor::from_slice(&bytes[..]).expect("bytes length is 32")
                },
            },
        })
    }
}

#[cfg(test)]
mod test {

    use elements::encode::{Decodable, Encodable};

    use crate::{update::DownloadTxResult, Update};

    use super::EncodableTxOutSecrets;

    #[test]
    fn test_empty_update() {
        let tip = lwk_test_util::liquid_block_1().header;
        let mut update = Update {
            new_txs: super::DownloadTxResult::default(),
            txid_height_new: Default::default(),
            txid_height_delete: Default::default(),
            timestamps: Default::default(),
            scripts: Default::default(),
            tip,
        };
        assert!(update.only_tip());
        update.timestamps.push((0, 0));
        assert!(!update.only_tip());
    }

    #[test]
    fn test_tx_out_secrets_roundtrip() {
        let secret = EncodableTxOutSecrets {
            inner: elements::TxOutSecrets::new(
                elements::AssetId::default(),
                elements::confidential::AssetBlindingFactor::zero(),
                1000,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
        };

        let mut vec = vec![];
        let len = secret.consensus_encode(&mut vec).unwrap();
        assert_eq!(len, 104);

        let back = EncodableTxOutSecrets::consensus_decode(&vec[..]).unwrap();
        assert_eq!(secret, back)
    }

    #[test]
    fn test_download_tx_result_roundtrip() {
        let tx_out_secret = elements::TxOutSecrets::new(
            elements::AssetId::default(),
            elements::confidential::AssetBlindingFactor::zero(),
            1000,
            elements::confidential::ValueBlindingFactor::zero(),
        );
        let mut txs = vec![];
        let mut unblinds = vec![];
        let tx = lwk_test_util::liquid_block_1().txdata.pop().unwrap();
        unblinds.push((tx.input[0].previous_output, tx_out_secret));

        txs.push((tx.txid(), tx));

        let result = DownloadTxResult { txs, unblinds };

        let mut vec = vec![];
        let len = result.consensus_encode(&mut vec).unwrap();
        assert_eq!(len, 1325);

        let back = DownloadTxResult::consensus_decode(&vec[..]).unwrap();
        assert_eq!(result, back)
    }
}
