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

    #[error("Transaction extraction from PSET failed")]
    TxExtractionFailed,
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
    use elements::bitcoin::bip32::DerivationPath;
    use elements::bitcoin::{secp256k1, PublicKey};
    use elements::encode::{deserialize, serialize};
    use elements::pset::{Input, Output};
    use elements::secp256k1_zkp::XOnlyPublicKey;
    use elements::taproot::TapLeafHash;
    use elements::{confidential, SchnorrSig, Script, TxOut, TxOutWitness};
    use std::str::FromStr;
    use std::sync::LazyLock;

    pub static EC: LazyLock<secp256k1::Secp256k1<secp256k1::All>> = LazyLock::new(|| {
        let mut ctx = secp256k1::Secp256k1::new();
        let mut rng = rand::thread_rng();
        ctx.randomize(&mut rng);
        ctx
    });

    const PK: &str = "020202020202020202020202020202020202020202020202020202020202020202";

    fn test_pset() -> PartiallySignedTransaction {
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut input = Input::default();
        let pk = PublicKey::from_str(PK).unwrap();
        input.bip32_derivation.insert(
            pk,
            (
                Fingerprint::from_str("aabbccdd").unwrap(),
                DerivationPath::master(),
            ),
        );
        pset.add_input(input);
        pset.add_output(Output {
            asset: Some(elements::AssetId::LIQUID_BTC),
            amount: Some(1000),
            ..Default::default()
        });
        pset
    }

    #[test]
    fn test_verify_added_sigs() {
        let fp = Fingerprint::from_str("aabbccdd").unwrap();
        let other_fp = Fingerprint::from_str("11223344").unwrap();

        let schnorr_sig = SchnorrSig::from_slice(&[0x12u8; 64]).unwrap();
        let other_schnorr_sig = SchnorrSig::from_slice(&[0x34u8; 64]).unwrap();
        let xonly = XOnlyPublicKey::from_slice(&[0x02u8; 32]).unwrap();

        let leaf = TapLeafHash::from_slice(&[0x04u8; 32]).unwrap();
        let other_leaf = TapLeafHash::from_slice(&[0x05u8; 32]).unwrap();

        let ecdsa_sig = vec![0x30, 0x45];
        let other_ecdsa_sig = vec![0x30, 0x46];
        let ecdsa_sig_too_short = vec![0x30, 0x46];
        let ecdsa_sig_mismatch_flag = vec![0x30, 0x01];

        let pk = PublicKey::from_str(PK).unwrap();
        let s = "030303030303030303030303030303030303030303030303030303030303030302";
        let other_pk = PublicKey::from_str(s).unwrap();

        // identical pset with no added signatures
        let original = test_pset();
        assert_eq!(
            verify_added_sigs(&original, &original.clone(), fp, &EC).unwrap(),
            0
        );

        // add partial sig from a key with no key origin
        let original = test_pset();
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(other_pk, ecdsa_sig.clone());
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::MissingKeyOrigin { idx: 0 }
        ));

        // added partial sig from a key with the wrong fingerprint
        let original = test_pset();
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig.clone());
        let err = verify_added_sigs(&original, &returned, other_fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::WrongFingerprint { idx: 0 }
        ));

        // removed partial sig
        let mut original = test_pset();
        original.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig.clone());
        let mut returned = original.clone();
        returned.inputs_mut()[0].partial_sigs.clear();
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::PartialSigsRemoved { idx: 0 }
        ));

        // changed partial sig
        let mut original = test_pset();
        original.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig.clone());
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(pk, other_ecdsa_sig.clone());
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::PartialSigsRemoved { idx: 0 }
        ));

        // partial sig whose sighash byte does not match the input's sighash_type
        let mut original = test_pset();
        original.inputs_mut()[0].sighash_type = Some(elements::pset::PsbtSighashType::from_u32(2));
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig_mismatch_flag);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::InvalidSignature));

        // partial sig on a taproot input: the sighash is not an ECDSA sighash
        let mut original = test_pset();
        original.inputs_mut()[0].witness_utxo = Some(TxOut {
            asset: confidential::Asset::Null,
            value: confidential::Value::Explicit(1000),
            nonce: confidential::Nonce::Null,
            script_pubkey: Script::new_v1_p2tr(&EC, xonly, None),
            witness: TxOutWitness::default(),
        });
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig.clone());
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::InvalidSignature));

        // partial sig that is too short to contain a sighash byte
        let original = test_pset();
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .partial_sigs
            .insert(pk, ecdsa_sig_too_short.clone());
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::InvalidSignature));

        // tap key sig with no internal key
        let original = test_pset();
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_key_sig = Some(schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::MissingInternalKey { idx: 0 }
        ));

        // tap key sig with internal key but no tap_key_origins entry
        let mut original = test_pset();
        original.inputs_mut()[0].tap_internal_key = Some(xonly);
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_key_sig = Some(schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::MissingTapKeyOrigin { idx: 0 }
        ));

        // tap key sig with wrong fingerprint
        let mut original = test_pset();
        original.inputs_mut()[0].tap_internal_key = Some(xonly);
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![], (other_fp, DerivationPath::master())));
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_key_sig = Some(schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::WrongFingerprint { idx: 0 }
        ));

        // changed tap key sig
        let mut original = test_pset();
        original.inputs_mut()[0].tap_internal_key = Some(xonly);
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![], (fp, DerivationPath::master())));
        original.inputs_mut()[0].tap_key_sig = Some(schnorr_sig);
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_key_sig = Some(other_schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::TapKeySigChanged { idx: 0 }
        ));

        // tap script sig with no tap_key_origins entry
        let original = test_pset();
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .tap_script_sigs
            .insert((xonly, leaf), schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::MissingTapKeyOrigin { idx: 0 }
        ));

        // tap script sig with wrong fingerprint
        let mut original = test_pset();
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![leaf], (other_fp, DerivationPath::master())));
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .tap_script_sigs
            .insert((xonly, leaf), schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::WrongFingerprint { idx: 0 }
        ));

        // tap script sig whose key origin lists leaves that don't include this leaf
        let mut original = test_pset();
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![other_leaf], (fp, DerivationPath::master())));
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .tap_script_sigs
            .insert((xonly, leaf), schnorr_sig);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::MissingTapKeyOrigin { idx: 0 }
        ));

        // removed tap script sig
        let mut original = test_pset();
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![leaf], (fp, DerivationPath::master())));
        original.inputs_mut()[0]
            .tap_script_sigs
            .insert((xonly, leaf), schnorr_sig);
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_script_sigs.clear();
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(
            err,
            PsetValidationError::TapScriptSigsRemoved { idx: 0 }
        ));

        // tap key sig with a correct fingerprint
        // TODO: remove this once taproot key spend signature verification is implemented
        let mut original = test_pset();
        original.inputs_mut()[0].tap_internal_key = Some(xonly);
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![], (fp, DerivationPath::master())));
        let mut returned = original.clone();
        returned.inputs_mut()[0].tap_key_sig = Some(schnorr_sig);
        assert_eq!(verify_added_sigs(&original, &returned, fp, &EC).unwrap(), 1);

        // tap script sig with a correct fingerprint
        // TODO: remove this once taproot script spend signature verification is implemented
        let mut original = test_pset();
        original.inputs_mut()[0]
            .tap_key_origins
            .insert(xonly, (vec![leaf], (fp, DerivationPath::master())));
        let mut returned = original.clone();
        returned.inputs_mut()[0]
            .tap_script_sigs
            .insert((xonly, leaf), schnorr_sig);
        assert_eq!(verify_added_sigs(&original, &returned, fp, &EC).unwrap(), 1);

        // non-signature fields are protected: any change triggers DataMismatch
        let original = test_pset();
        let base = original.clone();

        let mut returned = base.clone();
        returned.outputs_mut()[0].amount = Some(100_000);
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::DataMismatch));

        let mut returned = base.clone();
        returned.inputs_mut()[0].sighash_type = Some(elements::pset::PsbtSighashType::from_u32(1));
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::DataMismatch));

        let mut returned = base.clone();
        returned.add_input(Input::default());
        let err = verify_added_sigs(&original, &returned, fp, &EC).unwrap_err();
        assert!(matches!(err, PsetValidationError::DataMismatch));
    }

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
