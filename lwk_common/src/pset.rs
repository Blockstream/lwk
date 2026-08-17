use std::collections::BTreeMap;

use elements::{
    bitcoin::{bip32::Fingerprint, PublicKey},
    hashes::Hash,
    pset::{raw::ProprietaryKey, PartiallySignedTransaction},
    secp256k1_zkp::{ecdsa, Secp256k1},
    sighash::SighashCache,
    BlockHash, Transaction,
};
use elements_miniscript::psbt::{PsbtExt, PsbtSighashMsg};

use crate::Network;

const PSBT_ELEMENTS_GLOBAL_GENESIS_HASH: u8 = 0x02;

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum PsetValidationError {
    #[error("input #{idx}: partial signatures removed or changed")]
    PartialSigsRemoved { idx: usize },

    #[error("input #{idx}: tap key signature removed or changed")]
    TapKeySigChanged { idx: usize },

    #[error("input #{idx}: tap script signatures removed or changed")]
    TapScriptSigsRemoved { idx: usize },

    #[error("input #{idx}: added signature has no key origin")]
    MissingKeyOrigin { idx: usize },

    #[error("input #{idx}: added taproot signature has no internal key")]
    MissingInternalKey { idx: usize },

    #[error("input #{idx}: added taproot signature has no key origin")]
    MissingTapKeyOrigin { idx: usize },

    #[error("input #{idx}: added signature from a different key")]
    WrongFingerprint { idx: usize },

    #[error("PSET data differs between original and returned")]
    DataMismatch,

    #[error("added signature is invalid")]
    InvalidSignature,
}

// TODO: upstream to rust elements
/// Extract the genesis block hash from the PSET global proprietary fields as defined in
/// [ELIP-101](https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki).
///
/// Returns [`BlockHash::all_zeros`] if the field is absent or malformed.
pub fn get_genesis_hash(pset: &PartiallySignedTransaction) -> BlockHash {
    let key = ProprietaryKey::from_pset_pair(PSBT_ELEMENTS_GLOBAL_GENESIS_HASH, vec![]);
    pset.global
        .proprietary
        .get(&key)
        .and_then(|v| v.as_slice().try_into().ok())
        .map(BlockHash::from_byte_array)
        .unwrap_or(BlockHash::all_zeros())
}

// TODO: upstream to rust elements
// TODO: tested with Jade 1.0.37 but does not work. Safe to merge because subtype is unique.
/// Add genesis block hash as defined in [ELIP-101](https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki)
pub fn set_genesis_hash(pset: &mut PartiallySignedTransaction, network: &Network) {
    let genesis_block_hash = network.genesis_hash().to_byte_array().to_vec();

    pset.global.proprietary.insert(
        ProprietaryKey::from_pset_pair(PSBT_ELEMENTS_GLOBAL_GENESIS_HASH, vec![]),
        genesis_block_hash,
    );
}

/// Verify added signatures by a external cosigner.
///
/// Fails if:
/// * signed pset has changed, aside from signature for fingerprint (nothing removed, nothing added)
/// * there is any missing signature for fingerprint
/// * added signatures are invalid
///
/// **Experimental**: this API might change without notice.
pub fn verify_added_sigs<C: elements::secp256k1_zkp::Verification>(
    original: &PartiallySignedTransaction,
    returned: &PartiallySignedTransaction,
    fingerprint: Fingerprint,
    secp: &Secp256k1<C>,
) -> Result<usize, PsetValidationError> {
    let mut r = returned.clone();
    for (orig_in, ret_in) in original.inputs().iter().zip(r.inputs_mut()) {
        ret_in.partial_sigs = orig_in.partial_sigs.clone();
        ret_in.tap_key_sig = orig_in.tap_key_sig;
        ret_in.tap_script_sigs = orig_in.tap_script_sigs.clone();
    }

    if original != &r {
        return Err(PsetValidationError::DataMismatch);
    }

    let tx = returned
        .extract_tx()
        .map_err(|_| PsetValidationError::TxExtractionFailed)?;
    let mut env = VerifyEnv {
        pset: returned,
        cache: SighashCache::new(Box::new(tx)),
        genesis_hash: get_genesis_hash(returned),
        secp,
    };

    let mut added = 0;

    for (idx, (orig_in, ret_in)) in original.inputs().iter().zip(returned.inputs()).enumerate() {
        // verify partial signatures
        if !is_superset(&orig_in.partial_sigs, &ret_in.partial_sigs) {
            return Err(PsetValidationError::PartialSigsRemoved { idx });
        }

        for (pk, sig) in ret_in.partial_sigs.iter() {
            if !orig_in.partial_sigs.contains_key(pk) {
                let (fp, _) = ret_in
                    .bip32_derivation
                    .get(pk)
                    .ok_or(PsetValidationError::MissingKeyOrigin { idx })?;
                if fp != &fingerprint {
                    return Err(PsetValidationError::WrongFingerprint { idx });
                }

                env.verify_ecdsa(idx, pk, sig)?;
                added += 1;
            }
        }

        // verify taproot key spend
        if orig_in.tap_key_sig.is_some() && orig_in.tap_key_sig != ret_in.tap_key_sig {
            return Err(PsetValidationError::TapKeySigChanged { idx });
        }

        if let (Some(_), None) = (&ret_in.tap_key_sig, &orig_in.tap_key_sig) {
            let internal_key = ret_in
                .tap_internal_key
                .ok_or(PsetValidationError::MissingInternalKey { idx })?;
            let (_, (fp, _)) = ret_in
                .tap_key_origins
                .get(&internal_key)
                .ok_or(PsetValidationError::MissingTapKeyOrigin { idx })?;
            if fp != &fingerprint {
                return Err(PsetValidationError::WrongFingerprint { idx });
            }

            // TODO: verify taproot key spend signature
            added += 1;
        }

        // verify taproot script spend
        if !is_superset(&orig_in.tap_script_sigs, &ret_in.tap_script_sigs) {
            return Err(PsetValidationError::TapScriptSigsRemoved { idx });
        }

        for ((pk, leaf), _) in ret_in.tap_script_sigs.iter() {
            if !orig_in.tap_script_sigs.contains_key(&(*pk, *leaf)) {
                let (leaves, (fp, _)) = ret_in
                    .tap_key_origins
                    .get(pk)
                    .ok_or(PsetValidationError::MissingTapKeyOrigin { idx })?;
                if !leaves.is_empty() && !leaves.contains(leaf) {
                    return Err(PsetValidationError::MissingTapKeyOrigin { idx });
                }

                if fp != &fingerprint {
                    return Err(PsetValidationError::WrongFingerprint { idx });
                }

                // TODO: verify taproot script spend signatures
                added += 1;
            }
        }
    }

    Ok(added)
}

/// Shared context for the sighash-based signature verification.
struct VerifyEnv<'a, C: elements::secp256k1_zkp::Verification> {
    pset: &'a PartiallySignedTransaction,
    cache: SighashCache<Box<Transaction>>,
    genesis_hash: BlockHash,
    secp: &'a Secp256k1<C>,
}

impl<C: elements::secp256k1_zkp::Verification> VerifyEnv<'_, C> {
    fn verify_ecdsa(
        &mut self,
        idx: usize,
        pk: &PublicKey,
        sig: &[u8],
    ) -> Result<(), PsetValidationError> {
        if sig.len() <= 1 {
            return Err(PsetValidationError::InvalidSignature);
        }

        let sig_hash_byte = sig[sig.len() - 1];

        let input = &self.pset.inputs()[idx];
        if let Some(expected_sighash) = input.sighash_type {
            if expected_sighash.to_u32() as u8 != sig_hash_byte {
                return Err(PsetValidationError::InvalidSignature);
            }
        }

        let msg = self
            .pset
            .sighash_msg(idx, &mut self.cache, None, self.genesis_hash)
            .map_err(|_| PsetValidationError::InvalidSignature)?;

        if !matches!(&msg, PsbtSighashMsg::EcdsaSighash(_)) {
            return Err(PsetValidationError::InvalidSignature);
        }
        let der = ecdsa::Signature::from_der(&sig[..sig.len() - 1])
            .map_err(|_| PsetValidationError::InvalidSignature)?;
        self.secp
            .verify_ecdsa(&msg.to_secp_msg(), &der, &pk.inner)
            .map_err(|_| PsetValidationError::InvalidSignature)?;
        Ok(())
    }
}

/// Checks if `ret` contains every entry of `orig` with the same value
fn is_superset<K: Ord, V: PartialEq>(orig: &BTreeMap<K, V>, ret: &BTreeMap<K, V>) -> bool {
    orig.iter().all(|(k, v)| ret.get(k) == Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements::encode::{deserialize, serialize};

    #[test]
    fn test_genesis_hash_serde_roundtrip() {
        let network = Network::Liquid;
        let mut pset = PartiallySignedTransaction::new_v2();
        set_genesis_hash(&mut pset, &network);

        let serialized = serialize(&pset);
        let deserialized: PartiallySignedTransaction = deserialize(&serialized).unwrap();

        assert_eq!(get_genesis_hash(&deserialized), network.genesis_hash());
    }
}
