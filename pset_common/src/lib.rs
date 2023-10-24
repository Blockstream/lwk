mod error;
mod model;

pub use crate::error::Error;
pub use crate::model::*;

use elements_miniscript::confidential::bare::tweak_private_key;
use elements_miniscript::confidential::Key;
use elements_miniscript::descriptor::DescriptorSecretKey;
use elements_miniscript::elements::bitcoin::secp256k1::SecretKey;
use elements_miniscript::elements::{
    bitcoin::{bip32::KeySource, key::PublicKey},
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{All, Generator, PedersenCommitment, Secp256k1},
    AssetId, BlindAssetProofs, BlindValueProofs, OutPoint, Script, TxOutSecrets,
};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use std::collections::btree_map::BTreeMap;
use std::collections::HashMap;

pub fn derive_script_pubkey(
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    index: u32,
) -> Result<Script, Error> {
    Ok(descriptor
        .descriptor
        .at_derivation_index(index)?
        .script_pubkey())
}

pub fn derive_blinding_key(
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    script_pubkey: &Script,
) -> Option<SecretKey> {
    let secp = Secp256k1::new();
    match &descriptor.key {
        Key::Slip77(k) => Some(k.blinding_private_key(script_pubkey)),
        Key::View(DescriptorSecretKey::XPrv(dxk)) => {
            let k = dxk.xkey.to_priv();
            Some(tweak_private_key(&secp, script_pubkey, &k.inner))
        }
        Key::View(DescriptorSecretKey::Single(k)) => {
            Some(tweak_private_key(&secp, script_pubkey, &k.key.inner))
        }
        _ => None,
    }
}

fn commitments(
    secp: &Secp256k1<All>,
    txout_secrets: &TxOutSecrets,
) -> (Generator, PedersenCommitment) {
    let asset_comm = Generator::new_blinded(
        secp,
        txout_secrets.asset.into_inner().to_byte_array().into(),
        txout_secrets.asset_bf.into_inner(),
    );
    let amount_comm = PedersenCommitment::new(
        secp,
        txout_secrets.value,
        txout_secrets.value_bf.into_inner(),
        asset_comm,
    );
    (asset_comm, amount_comm)
}

fn is_mine(
    script_pubkey: &Script,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    bip32_derivation: &BTreeMap<PublicKey, KeySource>,
) -> bool {
    for (_, path) in bip32_derivation.values() {
        // TODO should I check descriptor derivation path is compatible with given bip32_derivation?
        // TODO consider fingerprint if available
        if path.is_empty() {
            continue;
        }
        let wildcard_index = path[path.len() - 1];
        let mine = derive_script_pubkey(descriptor, wildcard_index.into()).unwrap();
        if &mine == script_pubkey {
            return true;
        }
    }
    false
}

pub fn pset_balance(
    pset: &PartiallySignedTransaction,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<PsetBalance, Error> {
    let secp = Secp256k1::new();
    let mut balances: HashMap<AssetId, i64> = HashMap::new();
    let mut fee: Option<u64> = None;
    for (idx, input) in pset.inputs().iter().enumerate() {
        match input.witness_utxo.as_ref() {
            None => {
                let previous_outpoint = OutPoint {
                    txid: input.previous_txid,
                    vout: input.previous_output_index,
                };
                return Err(Error::MissingPreviousOutput {
                    idx,
                    previous_outpoint,
                });
            }
            Some(txout) => {
                if !is_mine(&txout.script_pubkey, descriptor, &input.bip32_derivation) {
                    // Ignore outputs we don't own
                    continue;
                }

                if input.is_pegin() {
                    return Err(Error::InputPeginUnsupported { idx });
                }
                if input.has_issuance() {
                    let issuance = input.asset_issuance();
                    if issuance.amount.is_confidential()
                        || issuance.inflation_keys.is_confidential()
                    {
                        return Err(Error::InputBlindedIssuance { idx });
                    }
                }

                // We expect the input to be blinded
                if !(txout.asset.is_confidential() && txout.value.is_confidential()) {
                    return Err(Error::InputNotBlinded { idx });
                }
                let asset_comm = txout.asset.commitment().unwrap();
                let amount_comm = txout.value.commitment().unwrap();

                // We expect the input to be unblindable with the descriptor blinding key
                let private_blinding_key = derive_blinding_key(descriptor, &txout.script_pubkey)
                    .ok_or(Error::MissingPrivateBlindingKey)?;
                let txout_secrets = txout
                    .unblind(&secp, private_blinding_key)
                    .map_err(|_| Error::InputMineNotUnblindable { idx })?;
                if (asset_comm, amount_comm) != commitments(&secp, &txout_secrets) {
                    return Err(Error::InputCommitmentsMismatch { idx });
                }

                *balances.entry(txout_secrets.asset).or_default() -= txout_secrets.value as i64;
            }
        }
    }

    for (idx, output) in pset.outputs().iter().enumerate() {
        if output.script_pubkey.is_empty() {
            // Candidate fee output
            if fee.is_some() {
                return Err(Error::MultipleFee);
            }
            if output.asset.is_none()
                || output.asset_comm.is_some()
                || output.amount.is_none()
                || output.amount_comm.is_some()
            {
                return Err(Error::BlindedFee);
            }
            fee = Some(output.amount.unwrap());
            continue;
        }

        if !is_mine(&output.script_pubkey, descriptor, &output.bip32_derivation) {
            // Ignore outputs we don't own
            continue;
        }

        // Expect all outputs to be blinded and with blind proofs
        match (
            output.asset,
            output.asset_comm,
            output.blind_asset_proof.as_ref(),
            output.amount,
            output.amount_comm,
            output.blind_value_proof.as_ref(),
        ) {
            (None, _, _, None, _, _) => return Err(Error::OutputAssetValueNone { idx }),
            (None, _, _, Some(_), _, _) => return Err(Error::OutputValueNone { idx }),
            (Some(_), _, _, None, _, _) => return Err(Error::OutputAssetNone { idx }),
            (
                Some(asset),
                Some(asset_comm),
                Some(blind_asset_proof),
                Some(amount),
                Some(amount_comm),
                Some(blind_value_proof),
            ) => {
                if !blind_asset_proof.blind_asset_proof_verify(&secp, asset, asset_comm) {
                    return Err(Error::InvalidAssetBlindProof { idx });
                }
                if !blind_value_proof.blind_value_proof_verify(
                    &secp,
                    amount,
                    asset_comm,
                    amount_comm,
                ) {
                    return Err(Error::InvalidValueBlindProof { idx });
                }

                // Check that we can later unblind the output
                let private_blinding_key = derive_blinding_key(descriptor, &output.script_pubkey)
                    .ok_or(Error::MissingPrivateBlindingKey)?;
                let txout_secrets = output
                    .to_txout()
                    .unblind(&secp, private_blinding_key)
                    .map_err(|_| Error::OutputMineNotUnblindable { idx })?;
                if (asset_comm, amount_comm) != commitments(&secp, &txout_secrets) {
                    return Err(Error::OutputCommitmentsMismatch { idx });
                }

                *balances.entry(asset).or_default() += amount as i64;
            }
            _ => return Err(Error::OutputNotBlinded { idx }),
        }
    }
    let fee = fee.ok_or(Error::MissingFee)?;

    Ok(PsetBalance { fee, balances })
}

pub fn pset_signatures(pset: &PartiallySignedTransaction) -> Vec<PsetSignatures> {
    pset.inputs()
        .iter()
        .map(|input| {
            let mut has_signature = vec![];
            let mut missing_signature = vec![];
            for (pk, ks) in input.bip32_derivation.clone() {
                if input.partial_sigs.contains_key(&pk) {
                    has_signature.push((pk, ks));
                } else {
                    missing_signature.push((pk, ks));
                }
            }
            PsetSignatures {
                has_signature,
                missing_signature,
            }
        })
        .collect()
}
