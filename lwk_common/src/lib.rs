#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! A crate containing common code used in multiple other crate in the workspace, such as:
//!
//!   * Utils to inspect a PSET: get the net effect of a PSET on a given wallet [`pset_balance()`], or get how many
//!     signatures are missing , and which signers should provide them [`pset_signatures()`].
//!  * [`Signer`] trait: contains the methods to be implemented by a signer such as signing a pset or
//!     returning an xpub
//!
//!  To avoid circular dependencies this crate must not depend on other crate of the workspace

mod descriptor;
mod error;
mod keyorigin_xpub;
mod model;
pub mod precision;
mod qr;
mod segwit;
mod signer;

pub use crate::descriptor::{
    multisig_desc, singlesig_desc, Bip, DescriptorBlindingKey, InvalidBipVariant,
    InvalidBlindingKeyVariant, InvalidMultisigVariant, InvalidSinglesigVariant, Multisig,
    Singlesig,
};
pub use crate::error::Error;
pub use crate::keyorigin_xpub::{keyorigin_xpub_from_str, InvalidKeyOriginXpub};
pub use crate::model::*;
pub use crate::precision::Precision;
pub use crate::qr::*;
pub use crate::segwit::is_provably_segwit;
pub use crate::signer::Signer;

use elements::confidential::{Asset, Value};
use elements_miniscript::confidential::bare::tweak_private_key;
use elements_miniscript::confidential::Key;
use elements_miniscript::descriptor::DescriptorSecretKey;
use elements_miniscript::elements::bitcoin::secp256k1::SecretKey;
use elements_miniscript::elements::{
    bitcoin::{bip32::KeySource, key::PublicKey},
    opcodes::all::OP_RETURN,
    pset::PartiallySignedTransaction,
    script::Builder,
    secp256k1_zkp::{All, Generator, PedersenCommitment, Secp256k1},
    AssetId, BlindAssetProofs, BlindValueProofs, OutPoint, Script, TxOutSecrets,
};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use std::collections::btree_map::BTreeMap;

pub mod electrum_ssl {
    pub const LIQUID_SOCKET: &str = "elements-mainnet.blockstream.info:50002";
    pub const LIQUID_TESTNET_SOCKET: &str = "elements-testnet.blockstream.info:50002";
}

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
) -> Result<bool, Error> {
    for (_, path) in bip32_derivation.values() {
        // TODO should I check descriptor derivation path is compatible with given bip32_derivation?
        // TODO consider fingerprint if available
        if path.is_empty() {
            continue;
        }
        let wildcard_index = path[path.len() - 1];
        for d in descriptor.descriptor.clone().into_single_descriptors()? {
            // TODO improve by checking only the descriptor ending with the given path
            let mine = d
                .at_derivation_index(wildcard_index.into())?
                .script_pubkey();
            if &mine == script_pubkey {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn pset_balance(
    pset: &PartiallySignedTransaction,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<PsetBalance, Error> {
    let secp = Secp256k1::new();
    let mut balances: BTreeMap<AssetId, i64> = BTreeMap::new();
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
                if !is_mine(&txout.script_pubkey, descriptor, &input.bip32_derivation)
                    .unwrap_or(false)
                {
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
                let (asset_comm, amount_comm) = match (txout.asset, txout.value) {
                    (Asset::Confidential(g), Value::Confidential(c)) => (g, c),
                    _ => return Err(Error::InputNotBlinded { idx }),
                };

                // We expect the input to be unblindable with the descriptor blinding key
                let private_blinding_key = derive_blinding_key(descriptor, &txout.script_pubkey)
                    .ok_or(Error::MissingPrivateBlindingKey)?;
                // However the rangeproof is stored in another field
                // since the output witness, which includes the rangeproof,
                // is not serialized.
                let mut txout_with_rangeproof = txout.clone();
                txout_with_rangeproof
                    .witness
                    .rangeproof
                    .clone_from(&input.in_utxo_rangeproof);
                let txout_secrets = txout_with_rangeproof
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
            fee = Some(output.amount.expect("previous if prevent this to be none"));
            continue;
        }

        if !is_mine(&output.script_pubkey, descriptor, &output.bip32_derivation).unwrap_or(false) {
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

pub fn pset_issuances(pset: &PartiallySignedTransaction) -> Vec<Issuance> {
    pset.inputs().iter().map(Issuance::new).collect()
}

/// Create the same burn script that Elements Core wallet creates
pub fn burn_script() -> Script {
    Builder::new().push_opcode(OP_RETURN).into_script()
}

#[cfg(test)]
mod test {
    use elements::{pset::PartiallySignedTransaction, AssetId};
    use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};

    use crate::pset_balance;

    #[test]
    fn test_pset_details() {
        let asset_id_str = "38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5";
        let asset_id: AssetId = asset_id_str.parse().unwrap();
        let desc_str = include_str!("../test_data/pset_details/descriptor");
        let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc_str.parse().unwrap();

        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc).unwrap();
        let v = balance.balances.get(&asset_id).unwrap();
        assert_eq!(*v, 0); // it's correct the balance of this asset 0 because it's a redeposit

        let pset_str = include_str!("../test_data/pset_details/pset2.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc).unwrap();
        let v = balance.balances.get(&asset_id).unwrap();
        assert_eq!(*v, -1);
    }
}
