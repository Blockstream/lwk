use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Timestamp};
use crate::wollet::WolletState;
use crate::EC;
use crate::{BlindingPublicKey, Wollet, WolletDescriptor};
use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::AeadMutInPlace;
use base64::prelude::*;
use elements::bitcoin::bip32::ChildNumber;
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::{Decodable, Encodable};
use elements::{BlockHeader, TxInWitness, TxOutWitness};
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::sync::atomic;

/// Transactions downloaded and unblinded
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct DownloadTxResult {
    /// Transactions downloaded
    pub txs: Vec<(Txid, Transaction)>,

    /// Unblinded outputs of the downloaded transactions
    pub unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

impl DownloadTxResult {
    fn is_empty(&self) -> bool {
        self.txs.is_empty() && self.unblinds.is_empty()
    }

    fn prune(&mut self, scripts: &HashMap<Script, (Chain, ChildNumber)>) {
        for (_, tx) in self.txs.iter_mut() {
            for input in tx.input.iter_mut() {
                input.witness = TxInWitness::empty();
            }

            for output in tx.output.iter_mut() {
                if scripts.contains_key(&output.script_pubkey) {
                    // we are keeping the rangeproof because it's needed for pset details
                    output.witness.surjection_proof = None;
                } else {
                    output.witness = TxOutWitness::empty();
                }
            }
        }
    }
}

/// Passing a wallet to [`crate::clients::blocking::BlockchainBackend::full_scan()`] returns this structure which
/// contains the delta of information to be applied to the wallet to reach the latest status.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Update {
    pub version: u8,

    /// The status of the wallet this update is generated from
    ///
    /// If 0 means it has been deserialized from a V0 version
    pub wollet_status: u64,

    pub new_txs: DownloadTxResult,
    pub txid_height_new: Vec<(Txid, Option<Height>)>,
    pub txid_height_delete: Vec<Txid>,
    pub timestamps: Vec<(Height, Timestamp)>,

    /// The script pub key with the chain, the child number and the blinding pubkey
    /// The blinding pubkey is optional for backward compatibility reasons
    pub scripts_with_blinding_pubkey: Vec<(Chain, ChildNumber, Script, Option<BlindingPublicKey>)>,
    pub tip: BlockHeader,
}

impl Update {
    pub fn only_tip(&self) -> bool {
        self.new_txs.is_empty()
            && self.txid_height_new.is_empty()
            && self.txid_height_delete.is_empty()
            && self.scripts_with_blinding_pubkey.is_empty()
    }
    pub fn prune(&mut self, wallet: &Wollet) {
        self.new_txs.prune(&wallet.store.cache.paths);
    }
    pub fn serialize(&self) -> Result<Vec<u8>, elements::encode::Error> {
        let mut vec = vec![];
        self.consensus_encode(&mut vec)?;
        Ok(vec)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Update, elements::encode::Error> {
        Update::consensus_decode(bytes)
    }

    pub fn serialize_encrypted(&self, desc: &WolletDescriptor) -> Result<Vec<u8>, Error> {
        let mut plaintext = self.serialize()?;

        let mut nonce_bytes = [0u8; 12];
        thread_rng().fill(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);

        desc.cipher().encrypt_in_place(nonce, b"", &mut plaintext)?;
        let ciphertext = plaintext;

        let mut result = Vec::with_capacity(ciphertext.len() + 12);
        result.extend(nonce.as_slice());
        result.extend(&ciphertext);

        Ok(result)
    }

    pub fn serialize_encrypted_base64(&self, desc: &WolletDescriptor) -> Result<String, Error> {
        let vec = self.serialize_encrypted(desc)?;
        Ok(BASE64_STANDARD.encode(vec))
    }

    pub fn deserialize_decrypted(bytes: &[u8], desc: &WolletDescriptor) -> Result<Update, Error> {
        let nonce_bytes = &bytes
            .get(..12)
            .ok_or_else(|| Error::Generic("Missing nonce in encrypted bytes".to_string()))?;
        let mut ciphertext = bytes[12..].to_vec();

        let nonce = GenericArray::from_slice(nonce_bytes);

        desc.cipher()
            .decrypt_in_place(nonce, b"", &mut ciphertext)?;
        let plaintext = ciphertext;

        Ok(Update::deserialize(&plaintext)?)
    }

    pub fn deserialize_decrypted_base64(
        base64: &str,
        desc: &WolletDescriptor,
    ) -> Result<Update, Error> {
        let vec = BASE64_STANDARD
            .decode(base64)
            .map_err(|e| Error::Generic(e.to_string()))?;
        Self::deserialize_decrypted(&vec, desc)
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

        if update.wollet_status != 0 {
            // wollet status 0 means the update has been created before saving the status (v0) and we can't check
            if self.wollet_status() != update.wollet_status {
                return Err(Error::UpdateOnDifferentStatus {
                    wollet_status: self.wollet_status(),
                    update_status: update.wollet_status,
                });
            }
        }
        let descriptor = self.wollet_descriptor();
        let store = &mut self.store;
        let Update {
            version: _,
            wollet_status: _,
            new_txs,
            txid_height_new,
            txid_height_delete,
            timestamps,
            scripts_with_blinding_pubkey,
            tip,
        } = update.clone();

        let scripts_with_blinding_pubkey =
            compute_blinding_pubkey_if_missing(scripts_with_blinding_pubkey, descriptor)?;

        if tip.height + 1 < store.cache.tip.0 {
            // Checking we are not applying an old update while giving enough space for a single block reorg
            return Err(Error::UpdateHeightTooOld {
                update_tip_height: tip.height,
                store_tip_height: store.cache.tip.0,
            });
        }

        store.cache.tip = (tip.height, tip.block_hash());
        store.cache.unblinded.extend(new_txs.unblinds);
        store.cache.all_txs.extend(new_txs.txs);
        store
            .cache
            .heights
            .retain(|k, _| !txid_height_delete.contains(k));
        store.cache.heights.extend(txid_height_new.clone());
        store.cache.timestamps.extend(timestamps);
        store.cache.scripts.extend(
            scripts_with_blinding_pubkey
                .clone()
                .into_iter()
                .map(|(a, b, c, d)| ((a, b), (c, d))),
        );
        store.cache.paths.extend(
            scripts_with_blinding_pubkey
                .clone()
                .into_iter()
                .map(|(a, b, c, _d)| (c, (a, b))),
        );
        let mut last_used_internal = None;
        let mut last_used_external = None;
        for (txid, _) in txid_height_new {
            if let Some(tx) = store.cache.all_txs.get(&txid) {
                for (vout, output) in tx.output.iter().enumerate() {
                    if !store
                        .cache
                        .unblinded
                        .contains_key(&OutPoint::new(txid, vout as u32))
                    {
                        // Output cannot be unblinded by wallet
                        continue;
                    }
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
                .fetch_max(last_used_external + 1, atomic::Ordering::Relaxed);
        }
        if let Some(last_used_internal) = last_used_internal {
            store
                .cache
                .last_unused_internal
                .fetch_max(last_used_internal + 1, atomic::Ordering::Relaxed);
        }

        if do_persist {
            self.persister.push(update)?;
        }

        Ok(())
    }
}

fn compute_blinding_pubkey_if_missing(
    scripts_with_blinding_pubkey: Vec<(
        Chain,
        ChildNumber,
        Script,
        Option<elements::secp256k1_zkp::PublicKey>,
    )>,
    wollet_descriptor: WolletDescriptor,
) -> Result<Vec<(Chain, ChildNumber, Script, BlindingPublicKey)>, Error> {
    let mut result = Vec::with_capacity(scripts_with_blinding_pubkey.len());

    for (chain, child_number, script_pubkey, maybe_blinding_pubkey) in scripts_with_blinding_pubkey
    {
        let blinding_pubkey = match maybe_blinding_pubkey {
            Some(pubkey) => pubkey,
            None => {
                let desc = wollet_descriptor.ct_definite_descriptor(chain, child_number.into())?;
                // TODO: derive the blinding pubkey from the descriptor blinding key and scriptpubkey
                //       (needs function in elements-miniscript)

                let address = desc.address(&EC, &elements::AddressParams::ELEMENTS)?; // we don't need the address, we need only the blinding pubkey, thus we can use any params
                address
                    .blinding_pubkey
                    .expect("blinding pubkey is present when using ct descriptors")
            }
        };
        result.push((chain, child_number, script_pubkey, blinding_pubkey));
    }

    Ok(result)
}

impl Encodable for DownloadTxResult {
    fn consensus_encode<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> Result<usize, elements::encode::Error> {
        let mut bytes_written = 0;

        let txs_len = self.txs.len();
        bytes_written += elements::encode::VarInt(txs_len as u64).consensus_encode(&mut w)?;
        for (_txid, tx) in self.txs.iter() {
            // Avoid serializing Txid since are re-computable from the tx
            bytes_written += tx.consensus_encode(&mut w)?;
        }

        let unblinds_len = self.unblinds.len();
        bytes_written += elements::encode::VarInt(unblinds_len as u64).consensus_encode(&mut w)?;
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
        let txs_len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
        for _ in 0..txs_len {
            let tx = Transaction::consensus_decode(&mut d)?;
            txs.push((tx.txid(), tx));
        }

        let mut unblinds = vec![];
        let unblinds_len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
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

        bytes_written += self.version.consensus_encode(&mut w)?; // Version

        if self.version >= 1 {
            bytes_written += self.wollet_status.consensus_encode(&mut w)?;
        }

        bytes_written += self.new_txs.consensus_encode(&mut w)?;

        bytes_written +=
            elements::encode::VarInt(self.txid_height_new.len() as u64).consensus_encode(&mut w)?;
        for (txid, height) in self.txid_height_new.iter() {
            bytes_written += txid.consensus_encode(&mut w)?;
            bytes_written += height.unwrap_or(u32::MAX).consensus_encode(&mut w)?;
        }

        bytes_written += elements::encode::VarInt(self.txid_height_delete.len() as u64)
            .consensus_encode(&mut w)?;
        for txid in self.txid_height_delete.iter() {
            bytes_written += txid.consensus_encode(&mut w)?;
        }

        bytes_written +=
            elements::encode::VarInt(self.timestamps.len() as u64).consensus_encode(&mut w)?;
        for (height, timestamp) in self.timestamps.iter() {
            bytes_written += height.consensus_encode(&mut w)?;
            bytes_written += timestamp.consensus_encode(&mut w)?;
        }

        bytes_written += elements::encode::VarInt(self.scripts_with_blinding_pubkey.len() as u64)
            .consensus_encode(&mut w)?;
        for (chain, child_number, script, blinding_pubkey) in
            self.scripts_with_blinding_pubkey.iter()
        {
            bytes_written += script.consensus_encode(&mut w)?;
            bytes_written += match chain {
                Chain::External => 0u8,
                Chain::Internal => 1u8,
            }
            .consensus_encode(&mut w)?;
            bytes_written += u32::from(*child_number).consensus_encode(&mut w)?;
            if self.version >= 2 {
                match blinding_pubkey {
                    Some(blinding_pubkey) => {
                        bytes_written += blinding_pubkey.serialize().consensus_encode(&mut w)?
                    }
                    None => {
                        return Err(elements::encode::Error::ParseFailed(
                            "Version 2 update with missing blinding pubkey",
                        ))
                    }
                }
            }
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
        if version > 2 {
            return Err(elements::encode::Error::ParseFailed("Unsupported version"));
        }
        let wollet_status = if version >= 1 {
            u64::consensus_decode(&mut d)?
        } else {
            0
        };

        let new_txs = DownloadTxResult::consensus_decode(&mut d)?;

        let txid_height_new = {
            let len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
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
            let len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(Txid::consensus_decode(&mut d)?);
            }
            vec
        };

        let timestamps = {
            let len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let h = u32::consensus_decode(&mut d)?;
                let t = u32::consensus_decode(&mut d)?;
                vec.push((h, t));
            }
            vec
        };

        let scripts_with_blinding_pubkey = {
            let len = elements::encode::VarInt::consensus_decode(&mut d)?.0;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let script = Script::consensus_decode(&mut d)?;
                let chain = match u8::consensus_decode(&mut d)? {
                    0 => Chain::External,
                    1 => Chain::Internal,
                    _ => return Err(elements::encode::Error::ParseFailed("Invalid chain")),
                };
                let child_number: ChildNumber = u32::consensus_decode(&mut d)?.into();
                let blinding_pubkey = if version == 2 {
                    Some(BlindingPublicKey::consensus_decode(&mut d)?)
                } else {
                    None
                };
                vec.push((chain, child_number, script, blinding_pubkey));
            }
            vec
        };

        let tip = BlockHeader::consensus_decode(&mut d)?;

        Ok(Self {
            version,
            wollet_status,
            new_txs,
            txid_height_new,
            txid_height_delete,
            timestamps,
            scripts_with_blinding_pubkey,
            tip,
        })
    }
}

#[cfg(test)]
mod test {

    use elements::{
        encode::{Decodable, Encodable},
        Script,
    };

    use crate::{update::DownloadTxResult, Chain, Update, Wollet, WolletDescriptor};

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
            version: 1,
            new_txs: super::DownloadTxResult::default(),
            txid_height_new: Default::default(),
            txid_height_delete: Default::default(),
            timestamps: Default::default(),
            scripts_with_blinding_pubkey: Default::default(),
            tip,
            wollet_status: 1,
        };
        assert!(update.only_tip());
        update
            .txid_height_delete
            .push(<elements::Txid as elements::hashes::Hash>::all_zeros());
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
        let scripts_with_blinding_pubkey =
            vec![(Chain::Internal, 3u32.into(), Script::default(), None)];
        // previous version of this test was misleading by inserting two elements in a map, you have only one element.

        let tip = lwk_test_util::liquid_block_1().header;
        let update = Update {
            version: 1,
            new_txs,
            txid_height_new: vec![(txid, None), (txid, Some(12))],
            txid_height_delete: vec![txid],
            timestamps: vec![(12, 44), (12, 44)],
            scripts_with_blinding_pubkey,
            tip,
            wollet_status: 1,
        };

        let mut vec = vec![];
        let len = update.consensus_encode(&mut vec).unwrap();
        // std::fs::write("/tmp/xx.hex", vec.to_hex()).unwrap();
        let exp_vec = lwk_test_util::update_test_vector_v1_bytes();

        assert_eq!(vec.len(), exp_vec.len());
        assert_eq!(vec, exp_vec);
        assert_eq!(len, 2850);
        assert_eq!(vec.len(), len);

        let back = Update::consensus_decode(&vec[..]).unwrap();
        assert_eq!(update, back)
    }

    #[test]
    fn test_update_backward_comp() {
        // Update can be deserialize from v0 or v1 blob, but in the first case the wallet_status will be 0.
        let v0 = lwk_test_util::update_test_vector_bytes();
        let v1 = lwk_test_util::update_test_vector_v1_bytes();

        let upd_from_v0 = Update::deserialize(&v0).unwrap();

        let mut upd_from_v1 = Update::deserialize(&v1).unwrap();
        assert_ne!(upd_from_v0, upd_from_v1);
        upd_from_v1.wollet_status = 0;
        upd_from_v1.version = 0; // now we save the version in the struct, thus to compare for equality we need this hack
        assert_eq!(upd_from_v0, upd_from_v1);
    }

    #[test]
    fn test_update_decription() {
        let update = Update::deserialize(&lwk_test_util::update_test_vector_bytes()).unwrap();
        let desc: WolletDescriptor = lwk_test_util::wollet_descriptor_string().parse().unwrap();
        let enc_bytes = lwk_test_util::update_test_vector_encrypted_bytes();
        let update_from_enc = Update::deserialize_decrypted(&enc_bytes, &desc).unwrap();
        assert_eq!(update, update_from_enc);

        let enc_bytes2 = lwk_test_util::update_test_vector_encrypted_bytes2();
        let desc2: WolletDescriptor = lwk_test_util::wollet_descriptor_string2().parse().unwrap();
        Update::deserialize_decrypted(&enc_bytes2, &desc2).unwrap();
    }

    #[test]
    fn test_update_base64() {
        let base64 = lwk_test_util::update_test_vector_encrypted_base64();
        let desc: WolletDescriptor = lwk_test_util::wollet_descriptor_string().parse().unwrap();

        let update = Update::deserialize_decrypted_base64(&base64, &desc).unwrap();
        let update_ser = update.serialize_encrypted_base64(&desc).unwrap();
        assert_ne!(base64, update_ser); // decrypted content is the same, but encryption is not deterministic

        let back = Update::deserialize_decrypted_base64(&update_ser, &desc).unwrap();
        assert_eq!(update, back)
    }

    #[test]
    fn test_update_prune() {
        let update_bytes = lwk_test_util::update_test_vector_2_bytes();
        let update = Update::deserialize(&update_bytes).unwrap();
        let desc: WolletDescriptor = lwk_test_util::wollet_descriptor_string().parse().unwrap();
        let wollet = Wollet::without_persist(crate::ElementsNetwork::LiquidTestnet, desc).unwrap();
        assert_eq!(update_bytes.len(), 18436);
        assert_eq!(update.serialize().unwrap().len(), 18436);
        let update_pruned = {
            let mut u = update.clone();
            u.prune(&wollet);
            u
        };
        assert_eq!(update_pruned.serialize().unwrap().len(), 1106);
        assert_eq!(update.new_txs.txs.len(), update_pruned.new_txs.txs.len());
        assert_eq!(update.new_txs.unblinds, update_pruned.new_txs.unblinds);
    }
}
