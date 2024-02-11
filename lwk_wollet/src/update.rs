use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Timestamp};
use crate::Wollet;
use elements::bitcoin::bip32::ChildNumber;
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
    pub scripts: HashMap<Script, (Chain, ChildNumber)>, // TODO should be Vec<(Script,(Chain,ChildNumber))>
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
    pub fn serialize(&self) -> Result<Vec<u8>, elements::encode::Error> {
        let mut vec = vec![];
        self.consensus_encode(&mut vec)?;
        Ok(vec)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Update, elements::encode::Error> {
        Update::consensus_decode(bytes)
    }
}

impl Wollet {
    pub fn apply_update(&mut self, update: Update) -> Result<(), Error> {
        self.apply_update_inner(update, true)
    }

    pub fn apply_update_no_persist(&mut self, update: Update) -> Result<(), Error> {
        self.apply_update_inner(update, false)
    }

    fn apply_update_inner(&mut self, update: Update, do_persist: bool) -> Result<(), Error> {
        // TODO should accept &Update

        let store = &mut self.store;
        let Update {
            new_txs,
            txid_height_new,
            txid_height_delete,
            timestamps,
            scripts,
            tip,
        } = update.clone();

        if tip.height + 1 < store.cache.tip.0 {
            // Checking we are not applying an old update while giving enough space for a single block reorg
            return Err(Error::UpdateHeightTooOld {
                update_tip_height: tip.height,
                store_tip_height: store.cache.tip.0,
            });
        }

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

        if do_persist {
            self.persister.push(update)?;
        }

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
                inner: *tx_out_secrets,
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
            .into_inner()
            .as_ref()
            .consensus_encode(&mut w)?;

        bytes_written += self.inner.value.consensus_encode(&mut w)?;

        bytes_written += self
            .inner
            .value_bf
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

const UPDATE_MAGIC_BYTES: [u8; 4] = [0x89, 0x61, 0xb8, 0xc8];
impl Encodable for Update {
    fn consensus_encode<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> Result<usize, elements::encode::Error> {
        let mut bytes_written = 0;

        bytes_written += UPDATE_MAGIC_BYTES.consensus_encode(&mut w)?; // Magic bytes
        bytes_written += 0u8.consensus_encode(&mut w)?; // Version

        bytes_written += self.new_txs.consensus_encode(&mut w)?;

        bytes_written +=
            elements::VarInt(self.txid_height_new.len() as u64).consensus_encode(&mut w)?;
        for (txid, height) in self.txid_height_new.iter() {
            bytes_written += txid.consensus_encode(&mut w)?;
            bytes_written += height.unwrap_or(u32::MAX).consensus_encode(&mut w)?;
        }

        bytes_written +=
            elements::VarInt(self.txid_height_delete.len() as u64).consensus_encode(&mut w)?;
        for txid in self.txid_height_delete.iter() {
            bytes_written += txid.consensus_encode(&mut w)?;
        }

        bytes_written += elements::VarInt(self.timestamps.len() as u64).consensus_encode(&mut w)?;
        for (height, timestamp) in self.timestamps.iter() {
            bytes_written += height.consensus_encode(&mut w)?;
            bytes_written += timestamp.consensus_encode(&mut w)?;
        }

        bytes_written += elements::VarInt(self.scripts.len() as u64).consensus_encode(&mut w)?;
        for (script, (chain, child_number)) in self.scripts.iter() {
            bytes_written += script.consensus_encode(&mut w)?;
            bytes_written += match chain {
                Chain::External => 0u8,
                Chain::Internal => 1u8,
            }
            .consensus_encode(&mut w)?;
            bytes_written += u32::from(*child_number).consensus_encode(&mut w)?;
        }

        bytes_written += self.tip.consensus_encode(&mut w)?;

        Ok(bytes_written)
    }
}

impl Decodable for Update {
    fn consensus_decode<D: std::io::Read>(mut d: D) -> Result<Self, elements::encode::Error> {
        let magic_bytes: [u8; 4] = Decodable::consensus_decode(&mut d)?;
        if magic_bytes != UPDATE_MAGIC_BYTES {
            return Err(elements::encode::Error::ParseFailed("Invalid magic bytes"));
        }

        let version = u8::consensus_decode(&mut d)?;
        if version != 0 {
            return Err(elements::encode::Error::ParseFailed("Unsupported version"));
        }

        let new_txs = DownloadTxResult::consensus_decode(&mut d)?;

        let txid_height_new = {
            let len = elements::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let txid = Txid::consensus_decode(&mut d)?;
                let height = match u32::consensus_decode(&mut d)? {
                    u32::MAX => None,
                    x => Some(x),
                };
                vec.push((txid, height))
            }
            vec
        };

        let txid_height_delete = {
            let len = elements::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(Txid::consensus_decode(&mut d)?);
            }
            vec
        };

        let timestamps = {
            let len = elements::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let h = u32::consensus_decode(&mut d)?;
                let t = u32::consensus_decode(&mut d)?;
                vec.push((h, t));
            }
            vec
        };

        let scripts = {
            let len = elements::VarInt::consensus_decode(&mut d)?.0;
            let mut map = HashMap::with_capacity(len as usize);
            for _ in 0..len {
                let script = Script::consensus_decode(&mut d)?;
                let chain = match u8::consensus_decode(&mut d)? {
                    0 => Chain::External,
                    1 => Chain::Internal,
                    _ => return Err(elements::encode::Error::ParseFailed("Invalid chain")),
                };
                let child_number: ChildNumber = u32::consensus_decode(&mut d)?.into();
                map.insert(script, (chain, child_number));
            }
            map
        };

        let tip = BlockHeader::consensus_decode(&mut d)?;

        Ok(Self {
            new_txs,
            txid_height_new,
            txid_height_delete,
            timestamps,
            scripts,
            tip,
        })
    }
}

#[cfg(test)]
mod test {

    use std::collections::HashMap;

    use elements::{
        encode::{Decodable, Encodable},
        Script,
    };

    use crate::{update::DownloadTxResult, Chain, Update};

    use super::EncodableTxOutSecrets;

    pub fn download_tx_result_test_vector() -> DownloadTxResult {
        // there are issue in moving this in test_util
        let tx_out_secret = lwk_test_util::tx_out_secrets_test_vector();
        let mut txs = vec![];
        let mut unblinds = vec![];
        let tx = lwk_test_util::liquid_block_1().txdata.pop().unwrap();
        unblinds.push((tx.input[0].previous_output, tx_out_secret));

        txs.push((tx.txid(), tx));

        DownloadTxResult { txs, unblinds }
    }

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
            inner: lwk_test_util::tx_out_secrets_test_vector(),
        };

        let mut vec = vec![];
        let len = secret.consensus_encode(&mut vec).unwrap();
        assert_eq!(lwk_test_util::tx_out_secrets_test_vector_bytes(), vec);
        assert_eq!(len, 104);
        assert_eq!(vec.len(), len);

        let back = EncodableTxOutSecrets::consensus_decode(&vec[..]).unwrap();
        assert_eq!(secret, back)
    }

    #[test]
    fn test_download_tx_result_roundtrip() {
        let result = download_tx_result_test_vector();
        let mut vec = vec![];
        let len = result.consensus_encode(&mut vec).unwrap();
        assert_eq!(len, 1325);
        assert_eq!(vec.len(), len);

        let back = DownloadTxResult::consensus_decode(&vec[..]).unwrap();
        assert_eq!(result, back)
    }

    #[test]
    fn test_update_roundtrip() {
        let txid = lwk_test_util::txid_test_vector();
        let new_txs = download_tx_result_test_vector();
        let mut scripts = HashMap::new();
        scripts.insert(Script::default(), (Chain::External, 0u32.into()));
        scripts.insert(Script::default(), (Chain::Internal, 3u32.into()));

        let tip = lwk_test_util::liquid_block_1().header;
        let update = Update {
            new_txs,
            txid_height_new: vec![(txid, None), (txid, Some(12))],
            txid_height_delete: vec![txid],
            timestamps: vec![(12, 44), (12, 44)],
            scripts,
            tip,
        };

        let mut vec = vec![];
        let len = update.consensus_encode(&mut vec).unwrap();
        assert_eq!(vec, lwk_test_util::update_test_vector_bytes());
        assert_eq!(len, 2842);
        assert_eq!(vec.len(), len);

        let back = Update::consensus_decode(&vec[..]).unwrap();
        assert_eq!(update, back)
    }
}
