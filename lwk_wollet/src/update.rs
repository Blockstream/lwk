use crate::cache::{Height, Timestamp};
use crate::clients::try_unblind;
use crate::descriptor::Chain;
use crate::elements::{OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::wollet::{update_key, WolletState};
use crate::EC;
use crate::{BlindingPublicKey, Wollet, WolletDescriptor};
use base64::prelude::*;
use elements::bitcoin::bip32::ChildNumber;
use elements::bitcoin::hashes::Hash;
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::{Decodable, Encodable};
use elements::hash_types::TxMerkleNode;
use elements::{BlockExtData, BlockHash, BlockHeader, TxInWitness, TxOutWitness};
use lwk_common::SignedBalance;
use lwk_common::{decrypt_with_nonce_prefix, encrypt_with_random_nonce};
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
    /// The version of the update
    pub version: u8,

    /// The status of the wallet this update is generated from
    ///
    /// If 0 means it has been deserialized from a V0 version
    pub wollet_status: u64,

    /// The new transactions
    pub new_txs: DownloadTxResult,

    /// The new transaction with confirmation heights (or None if not confirmed)
    pub txid_height_new: Vec<(Txid, Option<Height>)>,

    /// The transaction ids to delete, for example after a reorg or a replace by fee.
    pub txid_height_delete: Vec<Txid>,

    /// The timestamps of the transactions, more precisely the timestamp of the block containing the transaction
    pub timestamps: Vec<(Height, Timestamp)>,

    /// The script pub key with the chain, the child number and the blinding pubkey
    /// The blinding pubkey is optional for backward compatibility reasons
    pub scripts_with_blinding_pubkey: Vec<(Chain, ChildNumber, Script, Option<BlindingPublicKey>)>,

    /// The tip of the blockchain at the time the update was generated
    pub tip: BlockHeader,
}

impl Update {
    /// Whether this update only changes the tip
    pub fn only_tip(&self) -> bool {
        self.new_txs.is_empty()
            && self.txid_height_new.is_empty()
            && self.txid_height_delete.is_empty()
            && self.scripts_with_blinding_pubkey.is_empty()
    }

    /// Prune the update, removing unneeded data from transactions.
    ///
    /// Note: this function removes less data than
    /// [`Update::prune_witnesses()`] since it keeps the rangeproofs
    /// of the outputs the [`Wollet`] owns.
    pub fn prune(&mut self, wallet: &Wollet) {
        self.new_txs.prune(&wallet.cache.paths);
    }

    /// Prune witnesses from transactions
    ///
    /// Remove all input and output witnesses from transcations downloaded in
    /// this update. This reduces memory and storage usage significantly.
    ///
    /// However pruning witnesses has effects on functions that use those
    /// rangeproofs (which are part of output witness):
    /// * When building transactions, it's possible to ask for the addition of
    ///   input rangeproofs, using [`crate::TxBuilder::add_input_rangeproofs()`]
    ///   or [`crate::WolletTxBuilder::add_input_rangeproofs()`]; however if the
    ///   rangeproofs have been removed, they cannot be added to the created
    ///   PSET.
    /// * [`Wollet::unblind_utxos_with()`] cannot unblind utxos without
    ///   witnesses.
    /// * [`Wollet::reunblind()`] cannot unblind transactions without
    ///   witnesses.
    pub fn prune_witnesses(&mut self) {
        for (_, tx) in self.new_txs.txs.iter_mut() {
            for input in tx.input.iter_mut() {
                input.witness = TxInWitness::empty();
            }
            for output in tx.output.iter_mut() {
                output.witness = TxOutWitness::empty();
            }
        }
    }

    /// Serialize an [`Update`] to a byte array
    pub fn serialize(&self) -> Result<Vec<u8>, elements::encode::Error> {
        let mut vec = vec![];
        self.consensus_encode(&mut vec)?;
        Ok(vec)
    }

    /// Deserialize an [`Update`] from a byte array
    pub fn deserialize(bytes: &[u8]) -> Result<Update, elements::encode::Error> {
        Update::consensus_decode(bytes)
    }

    /// Serialize an update to a byte array, encrypted with a key derived from the descriptor. Decrypt using [`Self::deserialize_decrypted()`]
    #[allow(deprecated)]
    pub fn serialize_encrypted(&self, desc: &WolletDescriptor) -> Result<Vec<u8>, Error> {
        let plaintext = self.serialize()?;
        let mut cipher = desc.cipher();
        let ciphertext = encrypt_with_random_nonce(&mut cipher, &plaintext)?;
        Ok(ciphertext)
    }

    /// Serialize an update to a base64 encoded string, encrypted with a key derived from the descriptor. Decrypt using [`Self::deserialize_decrypted_base64()`]
    pub fn serialize_encrypted_base64(&self, desc: &WolletDescriptor) -> Result<String, Error> {
        let vec = self.serialize_encrypted(desc)?;
        Ok(BASE64_STANDARD.encode(vec))
    }

    /// Deserialize an update from a byte array, decrypted with a key derived from the descriptor. Create the byte array using [`Self::serialize_encrypted()`]
    #[allow(deprecated)]
    pub fn deserialize_decrypted(bytes: &[u8], desc: &WolletDescriptor) -> Result<Update, Error> {
        let mut cipher = desc.cipher();
        let plaintext = decrypt_with_nonce_prefix(&mut cipher, bytes)?;
        Ok(Update::deserialize(&plaintext)?)
    }

    /// Deserialize an update from a base64 encoded string, decrypted with a key derived from the descriptor. Create the base64 using [`Self::serialize_encrypted_base64()`]
    pub fn deserialize_decrypted_base64(
        base64: &str,
        desc: &WolletDescriptor,
    ) -> Result<Update, Error> {
        let vec = BASE64_STANDARD
            .decode(base64)
            .map_err(|e| Error::Generic(e.to_string()))?;
        Self::deserialize_decrypted(&vec, desc)
    }

    /// Merge another update into this one.
    ///
    /// This is used to squash multiple sequential updates into a single update.
    ///
    /// NOTE: it's caller responsibility to ensure that the following update is the next in sequence
    /// and updates are not mixed up.
    pub(crate) fn merge(&mut self, following: Update) {
        // Merge new transactions: add new ones, replace existing ones
        for (txid, tx) in following.new_txs.txs {
            self.new_txs.txs.retain(|(t, _)| *t != txid);
            self.new_txs.txs.push((txid, tx));
        }
        self.new_txs.unblinds.extend(following.new_txs.unblinds);

        // Merge txid_height_new: union with override (later wins)
        for (txid, height) in following.txid_height_new {
            self.txid_height_new.retain(|(t, _)| *t != txid);
            self.txid_height_new.push((txid, height));
        }

        // Remove deleted txids from txid_height_new
        for txid in &following.txid_height_delete {
            self.txid_height_new.retain(|(t, _)| t != txid);
        }

        // Merge deletes
        self.txid_height_delete.extend(following.txid_height_delete);

        // Merge timestamps and scripts
        self.timestamps.extend(following.timestamps);
        self.scripts_with_blinding_pubkey
            .extend(following.scripts_with_blinding_pubkey);

        // Update tip to other's tip
        self.tip = following.tip;

        // Update version to latest
        self.version = following.version;
    }
}

fn default_blockheader() -> BlockHeader {
    BlockHeader {
        version: 0,
        prev_blockhash: BlockHash::all_zeros(),
        merkle_root: TxMerkleNode::all_zeros(),
        time: 0,
        height: 0,
        ext: BlockExtData::default(),
    }
}

/// Update the wallet state from blockchain data
impl Wollet {
    fn apply_transaction_inner(
        &mut self,
        tx: Transaction,
        do_persist: bool,
    ) -> Result<SignedBalance, Error> {
        let initial_balance = self.balance()?;
        let mut unblinds = vec![];
        let txid = tx.txid();
        for (vout, output) in tx.output.iter().enumerate() {
            if self.cache.paths.contains_key(&output.script_pubkey) {
                let outpoint = OutPoint::new(txid, vout as u32);
                match try_unblind(output, &self.descriptor) {
                    Ok(unblinded) => {
                        unblinds.push((outpoint, unblinded));
                    }
                    Err(_) => {
                        log::info!("{outpoint} cannot unblind, ignoring (could be sender messed up with the blinding process)");
                    }
                }
            }
        }

        let update = Update {
            version: 2,
            wollet_status: self.status(),
            new_txs: DownloadTxResult {
                txs: vec![(txid, tx)],
                unblinds,
            },
            txid_height_new: vec![(txid, None)],
            txid_height_delete: vec![],
            timestamps: vec![],
            scripts_with_blinding_pubkey: vec![],
            tip: default_blockheader(),
        };

        self.apply_update_inner(update, do_persist)?;
        let final_balance = self.balance()?;
        Ok(final_balance - initial_balance)
    }

    /// Apply an update containing blockchain data
    ///
    /// To update the wallet you need to first obtain the blockchain data relevant for the wallet.
    /// This can be done using [`crate::clients::blocking::BlockchainBackend::full_scan()`], which
    /// returns an [`crate::Update`] that contains new transaction and other data relevant for the
    /// wallet.
    /// The update must then be applied to the [`crate::Wollet`] so that wollet methods such as
    /// [`crate::Wollet::balance()`] or [`crate::Wollet::transactions()`] include the new data.
    ///
    /// However getting blockchain data involves network calls, so between the full scan start and
    /// when the update is applied it might elapse a significant amount of time.
    /// In that interval, applying any update, or any transaction using [`Wollet::apply_transaction()`],
    /// will cause this function to return a [`Error::UpdateOnDifferentStatus`].
    /// Callers should either avoid applying updates and transactions, or they can catch the error and wait for a new full scan to be completed and applied.
    pub fn apply_update(&mut self, update: Update) -> Result<(), Error> {
        self.apply_update_inner(update, true)
    }

    /// Same as [`Wollet::apply_update()`] but only apply the update in memory, without persisting it.
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
        let cache = &mut self.cache;
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

        if tip != default_blockheader() {
            if tip.height + 1 < cache.tip.0 {
                // Checking we are not applying an old update while giving enough space for a single block reorg
                return Err(Error::UpdateHeightTooOld {
                    update_tip_height: tip.height,
                    cache_tip_height: cache.tip.0,
                });
            }

            cache.tip = (tip.height, tip.block_hash());
        }

        cache.unblinded.extend(new_txs.unblinds);
        cache.all_txs.extend(new_txs.txs);
        cache.heights.retain(|k, _| !txid_height_delete.contains(k));
        cache.heights.extend(txid_height_new.clone());
        cache.timestamps.extend(timestamps);
        cache.scripts.extend(
            scripts_with_blinding_pubkey
                .clone()
                .into_iter()
                .map(|(a, b, c, d)| ((a, b), (c, d))),
        );
        cache.paths.extend(
            scripts_with_blinding_pubkey
                .clone()
                .into_iter()
                .map(|(a, b, c, _d)| (c, (a, b))),
        );
        let mut last_used_internal = None;
        let mut last_used_external = None;
        for (txid, _) in txid_height_new {
            if let Some(tx) = cache.all_txs.get(&txid) {
                for (vout, output) in tx.output.iter().enumerate() {
                    if !cache
                        .unblinded
                        .contains_key(&OutPoint::new(txid, vout as u32))
                    {
                        // Output cannot be unblinded by wallet
                        continue;
                    }
                    if let Some((ext_int, ChildNumber::Normal { index })) =
                        cache.paths.get(&output.script_pubkey)
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
            cache
                .last_unused_external
                .fetch_max(last_used_external + 1, atomic::Ordering::Relaxed);
        }
        if let Some(last_used_internal) = last_used_internal {
            cache
                .last_unused_internal
                .fetch_max(last_used_internal + 1, atomic::Ordering::Relaxed);
        }

        if do_persist {
            self.persist_update(update)?;
        }

        Ok(())
    }

    /// Persist an update to the store using an indexed key
    fn persist_update(&self, mut update: Update) -> Result<(), Error> {
        let mut next_index = self
            .next_update_index
            .lock()
            .map_err(|_| Error::Generic("next_update_index lock poisoned".into()))?;

        // Check if we can coalesce with the previous update (both are "only tip" updates)
        if update.only_tip() && *next_index > 0 {
            let prev_key = update_key(*next_index - 1);
            if let Ok(Some(prev_bytes)) = self.store.get(&prev_key) {
                if let Ok(prev_update) = Update::deserialize(&prev_bytes) {
                    if prev_update.only_tip() {
                        // Coalesce: overwrite the previous update
                        // Keep the previous wollet status so reapplying works correctly
                        update.wollet_status = prev_update.wollet_status;
                        // Merge timestamps
                        update.timestamps = [prev_update.timestamps, update.timestamps].concat();

                        let bytes = update.serialize()?;
                        self.store
                            .put(&prev_key, &bytes)
                            .map_err(|e| Error::Generic(format!("store error: {e}")))?;
                        return Ok(());
                    }
                }
            }
        }

        // Store as a new update
        let key = update_key(*next_index);
        let bytes = update.serialize()?;
        self.store
            .put(&key, &bytes)
            .map_err(|e| Error::Generic(format!("store error: {e}")))?;
        *next_index += 1;

        Ok(())
    }

    /// Apply a transaction to the wallet state
    ///
    /// Wallet transactions are normally obtained using [`crate::clients::blocking::BlockchainBackend::full_scan()`]
    /// and applying the resulting [`crate::Update`] with [`Wollet::apply_update()`]. However a
    /// full scan involves network calls and it can take a significant amount of time.
    ///
    /// If the caller does not want to wait for a full scan containing the transaction, it can
    /// apply the transaction to the wallet state using this function.
    ///
    /// Note: if this transaction is *not* returned by a next full scan, after [`Wollet::apply_update()`] it will disappear from the
    /// transactions list, will not be included in balance computations, and by the remaining
    /// wollet methods.
    ///
    /// Calling this method, might cause [`Wollet::apply_update()`] to fail with a
    /// [`Error::UpdateOnDifferentStatus`], make sure to either avoid it or handle the error properly.
    pub fn apply_transaction(&mut self, tx: Transaction) -> Result<SignedBalance, Error> {
        self.apply_transaction_inner(tx, true)
    }

    /// Same as [`Wollet::apply_transaction()`] but only apply the update in memory, without persisting it.
    pub fn apply_transaction_no_persist(
        &mut self,
        tx: Transaction,
    ) -> Result<SignedBalance, Error> {
        self.apply_transaction_inner(tx, false)
    }
}

#[allow(clippy::type_complexity)]
fn compute_blinding_pubkey_if_missing(
    scripts_with_blinding_pubkey: Vec<(
        Chain,
        ChildNumber,
        Script,
        Option<elements::secp256k1_zkp::PublicKey>,
    )>,
    wollet_descriptor: WolletDescriptor,
) -> Result<Vec<(Chain, ChildNumber, Script, Option<BlindingPublicKey>)>, Error> {
    let mut result = Vec::with_capacity(scripts_with_blinding_pubkey.len());

    for (chain, child_number, script_pubkey, maybe_blinding_pubkey) in scripts_with_blinding_pubkey
    {
        let blinding_pubkey = match maybe_blinding_pubkey {
            Some(pubkey) => Some(pubkey),
            None => {
                match wollet_descriptor.ct_definite_descriptor(chain, child_number.into()) {
                    Ok(desc) => {
                        // TODO: derive the blinding pubkey from the descriptor blinding key and scriptpubkey
                        //       (needs function in elements-miniscript)

                        let address = desc.address(&EC, &elements::AddressParams::ELEMENTS)?; // we don't need the address, we need only the blinding pubkey, thus we can use any params
                        Some(
                            address
                                .blinding_pubkey
                                .expect("blinding pubkey is present when using ct descriptors"),
                        )
                    }
                    Err(Error::UnsupportedWithoutDescriptor) => None,
                    Err(e) => return Err(e),
                }
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
                        bytes_written += [0u8; 33].consensus_encode(&mut w)?;
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
                    let bytes: [u8; 33] = Decodable::consensus_decode(&mut d)?;
                    if bytes == [0u8; 33] {
                        None
                    } else {
                        Some(BlindingPublicKey::from_slice(&bytes)?)
                    }
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
