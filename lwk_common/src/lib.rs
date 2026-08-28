#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

//! A crate containing common code used in multiple other crate in the workspace, such as:
//!
//!  * Utils to inspect a PSET: get the net effect of a PSET on a given wallet [`pset_balance()`], or get how many
//!    signatures are missing , and which signers should provide them [`pset_signatures()`].
//!  * [`Signer`] trait: contains the methods to be implemented by a signer such as signing a pset or
//!    returning an xpub
//!
//!  To avoid circular dependencies this crate must not depend on other crate of the workspace

mod address;
mod balance;
mod descriptor;
mod dpk;
mod encrypt;
mod error;
mod fee;
mod keyorigin_xpub;
mod model;
mod network;
mod output;
pub mod precision;
mod pset;
mod qr;
mod segwit;
mod signer;
#[cfg(all(feature = "sqlite", not(target_arch = "wasm32")))]
pub mod sqlite;
mod store;

pub use crate::address::{Address, AddressParseError};
pub use crate::balance::{Balance, SignedBalance};
pub use crate::descriptor::{
    multisig_desc, singlesig_desc, Bip, DescriptorBlindingKey, InvalidBipVariant,
    InvalidBlindingKeyVariant, InvalidMultisigVariant, InvalidSinglesigVariant, Multisig,
    Singlesig,
};
pub use crate::dpk::DescriptorPublicKey;
pub use crate::encrypt::{
    cipher_from_key_bytes, decrypt_with_nonce_prefix, encrypt_with_deterministic_nonce,
    encrypt_with_random_nonce, EncryptError,
};
pub use crate::error::Error;
pub use crate::keyorigin_xpub::{keyorigin_xpub_from_str, InvalidKeyOriginXpub};
pub use crate::model::*;
pub use crate::network::{ElementsParamsBuilder, Network};
pub use crate::precision::Precision;
pub use crate::pset::{get_genesis_hash, set_genesis_hash, verify_added_sigs, PsetValidationError};
pub use crate::qr::*;
pub use crate::segwit::is_provably_segwit;
#[cfg(feature = "amp0")]
pub use crate::signer::amp0::{Amp0Signer, Amp0SignerData};
pub use crate::signer::{ss_path, SSAccountType, Signer};
#[cfg(all(feature = "sqlite", not(target_arch = "wasm32")))]
pub use crate::sqlite::{SqliteStore, SqliteStoreError};
pub use crate::store::{
    ArcDynStoreError, BoxError, DynStore, EncryptedStore, EncryptedStoreError, FakeStore,
    FileStore, MemoryStore, Store,
};
pub use fee::*;
pub use output::OutputDetails;

/// A trait for async read/write operations used by hardware wallet connections
pub trait Stream {
    /// The error type returned by read and write operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Read data from the stream into the provided buffer
    fn read(&self, buf: &mut [u8])
        -> impl std::future::Future<Output = Result<usize, Self::Error>>;
    /// Write data to the stream
    fn write(&self, data: &[u8]) -> impl std::future::Future<Output = Result<(), Self::Error>>;
}

use elements::confidential::{Asset, Value};
use elements_miniscript::confidential::bare::tweak_private_key;
use elements_miniscript::confidential::Key;
use elements_miniscript::descriptor::DescriptorSecretKey;
use elements_miniscript::elements::bitcoin::secp256k1::SecretKey;
use elements_miniscript::elements::{
    opcodes::all::OP_RETURN,
    pset::PartiallySignedTransaction,
    script::Builder,
    secp256k1_zkp::{All, Generator, PedersenCommitment, Secp256k1},
    AssetId, BlindAssetProofs, BlindValueProofs, EcdsaSighashType, OutPoint, SchnorrSighashType,
    Script, TxOutSecrets,
};
use elements_miniscript::{
    ConfidentialDescriptor, DescriptorPublicKey as MiniscriptDescriptorPublicKey,
};
use output::is_mine;
use std::collections::btree_map::BTreeMap;
use std::collections::HashMap;

/// The sockets of the Blockstream Liquid Electrum servers.
pub mod electrum_ssl {
    /// The socket of the Blockstream Liquid mainnet Electrum server.
    pub const LIQUID_SOCKET: &str = "elements-mainnet.blockstream.info:50002";
    /// The socket of the Blockstream Liquid testnet Electrum server.
    pub const LIQUID_TESTNET_SOCKET: &str = "elements-testnet.blockstream.info:50002";
}

/// Derive the script pubkey from a confidential descriptor and an index.
pub fn derive_script_pubkey(
    descriptor: &ConfidentialDescriptor<MiniscriptDescriptorPublicKey>,
    index: u32,
) -> Result<Script, Error> {
    Ok(descriptor
        .descriptor
        .at_derivation_index(index)?
        .script_pubkey())
}

/// Derive the blinding secret key from a confidential descriptor and a script pubkey.
pub fn derive_blinding_key(
    descriptor: &ConfidentialDescriptor<MiniscriptDescriptorPublicKey>,
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

/// Return the net balance of a PSET from the perspective of the given `descriptor`.
/// It returns also the fee and the recipients (external receivers) of the PSET.
pub fn pset_balance(
    pset: &PartiallySignedTransaction,
    descriptor: &ConfidentialDescriptor<MiniscriptDescriptorPublicKey>,
    params: &'static elements::AddressParams,
) -> Result<PsetBalance, Error> {
    let secp = Secp256k1::new();
    let mut balances: BTreeMap<AssetId, i64> = BTreeMap::new();
    let mut fees: HashMap<AssetId, u64> = HashMap::new();
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
                if !is_mine(
                    &txout.script_pubkey,
                    descriptor,
                    &input.bip32_derivation,
                    &input.tap_key_origins,
                )
                .map(|(is_owned, _)| is_owned)
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

                let (asset, value) = match (
                    input.blind_asset_proof.as_ref(),
                    input.blind_value_proof.as_ref(),
                    input.asset,
                    input.amount,
                ) {
                    (Some(bap), Some(bvp), Some(asset), Some(value)) => {
                        if !bap.blind_asset_proof_verify(&secp, asset, asset_comm) {
                            return Err(Error::InvalidAssetBlindProof { idx });
                        }
                        if !bvp.blind_value_proof_verify(&secp, value, asset_comm, amount_comm) {
                            return Err(Error::InvalidValueBlindProof { idx });
                        }
                        (asset, value)
                    }
                    _ => {
                        // To handle PSETs created before we started adding input blind proofs,
                        // we also try to unblind the input with the descriptor blinding key
                        let private_blinding_key =
                            derive_blinding_key(descriptor, &txout.script_pubkey)
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
                        (txout_secrets.asset, txout_secrets.value)
                    }
                };

                *balances.entry(asset).or_default() -= value as i64;
            }
        }
    }

    let mut recipients = vec![];
    for (idx, output) in pset.outputs().iter().enumerate() {
        if output.script_pubkey.is_empty() {
            // Candidate fee output
            if output.asset.is_none()
                || output.asset_comm.is_some()
                || output.amount.is_none()
                || output.amount_comm.is_some()
            {
                return Err(Error::BlindedFee);
            }
            let asset = output.asset.expect("previous if prevent this to be none");
            let entry = fees.entry(asset).or_insert(0);
            *entry += output.amount.expect("previous if prevent this to be none");
            continue;
        }

        if !is_mine(
            &output.script_pubkey,
            descriptor,
            &output.bip32_derivation,
            &output.tap_key_origins,
        )
        .map(|(is_owned, _)| is_owned)
        .unwrap_or(false)
        {
            // external recipients
            let blinding_pubkey = output.blinding_key.as_ref().map(|k| k.inner);
            let address =
                elements::Address::from_script(&output.script_pubkey, blinding_pubkey, params);
            let recipient = Recipient::new(address, output.asset, output.amount, idx as u32);

            recipients.push(recipient);

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

    // Remove assets with 0 balance which are not changing the net balance.
    // For example it happens with reissuance tokens.
    balances.retain(|_, v| *v != 0);

    Ok(PsetBalance::new(fees, balances.into(), recipients))
}

/// Return the signatures of a PSET, for each input return a [`PsetSignatures`] which includes a
/// list of signatures that are available and a list of signatures that are missing.
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
            PsetSignatures::new(has_signature, missing_signature)
        })
        .collect()
}

fn input_spent_script_pubkey(input: &elements::pset::Input) -> Option<&Script> {
    if let Some(prevout) = input.witness_utxo.as_ref() {
        Some(&prevout.script_pubkey)
    } else if let Some(tx) = input.non_witness_utxo.as_ref() {
        tx.output
            .get(input.previous_output_index as usize)
            .map(|o| &o.script_pubkey)
    } else {
        None
    }
}

/// Return the sighash declared by an input, the default implied by the spent output script, or `None` if the spent output is unknown.
pub fn input_sighash(input: &elements::pset::Input) -> Option<u32> {
    if let Some(sighash) = input.sighash_type {
        return Some(sighash.to_u32());
    }

    input_spent_script_pubkey(input).map(|s| if s.is_v1_p2tr() { 0 } else { 1 })
}

fn input_has_non_default_sighash(input: &elements::pset::Input) -> bool {
    let Some(sighash) = input.sighash_type else {
        return false;
    };
    let Some(spk) = input_spent_script_pubkey(input) else {
        return false;
    };

    if spk.is_v1_p2tr() {
        // SIGHASH_DEFAULT and SIGHASH_ALL commit to the same data, but only the former can be
        // implied, so a taproot input declaring SIGHASH_ALL is still a default one.
        !matches!(
            sighash.schnorr_hash_ty(),
            Some(SchnorrSighashType::Default) | Some(SchnorrSighashType::All)
        )
    } else {
        !matches!(sighash.ecdsa_hash_ty(), Some(EcdsaSighashType::All))
    }
}

/// Whether any PSET input sighash is not the default one.
pub(crate) fn pset_has_non_default_sighash(pset: &PartiallySignedTransaction) -> bool {
    pset.inputs().iter().any(input_has_non_default_sighash)
}

/// Return the issuances of a PSET, for each input return an Issuance but the struct must be checked with [`Issuance::is_issuance`] if it's a real issuance or reissuance.
pub fn pset_issuances(pset: &PartiallySignedTransaction) -> Vec<Issuance> {
    pset.inputs().iter().map(Issuance::new).collect()
}

/// Create the same burn script that Elements Core wallet creates
pub fn burn_script() -> Script {
    Builder::new().push_opcode(OP_RETURN).into_script()
}

/// Create a debug string of a PSET, but remove new lines on number arrays.
pub fn pset_debug(pset: &PartiallySignedTransaction) -> String {
    let debug_str = format!("{pset:#?}");
    let mut result = String::new();

    // Remove new line for lines that contain only a number ending with a comma so that the output is more readable
    for line in debug_str.lines() {
        let trimmed = line.trim();

        if trimmed.ends_with(',') {
            if let Some(num_part) = trimmed.strip_suffix(",") {
                if num_part.parse::<u64>().is_ok() {
                    result.push_str(trimmed);
                    continue;
                }
            }
        }

        // For other lines, append with newline
        result.push_str(line);
        result.push('\n');
    }

    result
}

#[cfg(test)]
mod test {
    use elements::{pset::PartiallySignedTransaction, AssetId};
    use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};

    use crate::{input_sighash, pset_balance, pset_has_non_default_sighash};

    fn normalize_newlines(s: &str) -> String {
        s.replace("\r\n", "\n")
    }

    #[test]
    fn test_input_sighash() {
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let mut input = pset.inputs()[0].clone();

        // p2wsh input declaring nothing defaults to `ALL`
        assert!(input.sighash_type.is_none());
        assert_eq!(input_sighash(&input), Some(1));

        // the declared value is reported as is, even when non standard
        input.sighash_type = Some(elements::pset::PsbtSighashType::from_u32(131));
        assert_eq!(input_sighash(&input), Some(131));
        input.sighash_type = Some(elements::pset::PsbtSighashType::from_u32(153));
        assert_eq!(input_sighash(&input), Some(153));

        // a taproot input declaring nothing defaults to `DEFAULT`
        input.sighash_type = None;
        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend([0u8; 32]);
        let p2tr = elements::Script::from(p2tr);
        assert!(p2tr.is_v1_p2tr());
        input.witness_utxo.as_mut().unwrap().script_pubkey = p2tr.clone();
        assert_eq!(input_sighash(&input), Some(0));

        // the spent output is taken from `non_witness_utxo` when there is no `witness_utxo`
        let mut tx = elements::Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![],
            output: vec![elements::TxOut::default(), elements::TxOut::default()],
        };
        tx.output[1].script_pubkey = p2tr;
        input.witness_utxo = None;
        input.non_witness_utxo = Some(tx);
        input.previous_output_index = 1;
        assert_eq!(input_sighash(&input), Some(0));

        // without any spent output the input cannot be signed, return None
        input.non_witness_utxo = None;
        assert_eq!(input_sighash(&input), None);
    }

    #[test]
    fn test_pset_has_non_default_sighash() {
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();

        // an input declaring nothing commits to everything, whatever the spent output is
        assert!(!pset_has_non_default_sighash(&pset));

        let declare = |sighash: u32| {
            let mut pset = pset.clone();
            pset.inputs_mut()[0].sighash_type =
                Some(elements::pset::PsbtSighashType::from_u32(sighash));
            pset
        };

        // p2wsh input: only `ALL` commits to everything
        assert!(!pset_has_non_default_sighash(&declare(1)));
        assert!(pset_has_non_default_sighash(&declare(131)));

        // non standard values cannot be interpreted, warn about them
        assert!(pset_has_non_default_sighash(&declare(153)));

        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend([0u8; 32]);
        let p2tr = elements::Script::from(p2tr);
        assert!(p2tr.is_v1_p2tr());
        let declare_taproot = |sighash: u32| {
            let mut pset = declare(sighash);
            pset.inputs_mut()[0]
                .witness_utxo
                .as_mut()
                .unwrap()
                .script_pubkey = p2tr.clone();
            pset
        };

        // taproot input: `DEFAULT` and `ALL` commit to the same data
        assert!(!pset_has_non_default_sighash(&declare_taproot(0)));
        assert!(!pset_has_non_default_sighash(&declare_taproot(1)));
        assert!(pset_has_non_default_sighash(&declare_taproot(3)));

        // without the spent output the script type is unknown, don't warn
        let mut pset = declare(131);
        pset.inputs_mut()[0].witness_utxo = None;
        assert!(!pset_has_non_default_sighash(&pset));
    }

    fn setup_pset_details() -> (AssetId, ConfidentialDescriptor<DescriptorPublicKey>) {
        let asset_id_str = "38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5";
        let asset_id: AssetId = asset_id_str.parse().unwrap();
        let desc_str = include_str!("../test_data/pset_details/descriptor");
        let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc_str.parse().unwrap();
        (asset_id, desc)
    }

    #[test]
    fn test_pset_details_redeposit_balance() {
        let (asset_id, desc) = setup_pset_details();
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID_TESTNET).unwrap();
        assert!(
            !balance.balances().contains_key(&asset_id),
            "redeposit (balance = 0) should disappear from the list"
        );

        // Same for newly created psets with blind proofs
        let pset_str =
            include_str!("../test_data/pset_details/pset_with_input_blind_proofs.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID_TESTNET).unwrap();
        assert!(
            !balance.balances().contains_key(&asset_id),
            "redeposit (balance = 0) should disappear from the list"
        );
    }

    #[test]
    fn test_pset_details_negative_balance() {
        let (asset_id, desc) = setup_pset_details();
        let pset_str = include_str!("../test_data/pset_details/pset2.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID_TESTNET).unwrap();
        let v = balance.balances().get(&asset_id).unwrap();
        assert_eq!(*v, -1);

        // Same for newly created psets with blind proofs
        let pset_str =
            include_str!("../test_data/pset_details/pset2_with_input_blind_proofs.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID_TESTNET).unwrap();
        let v = balance.balances().get(&asset_id).unwrap();
        assert_eq!(*v, -1);
    }

    #[test]
    fn test_pset_outputs() {
        let pset_str = include_str!("../test_data/pset_outputs/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let desc_str = include_str!("../test_data/pset_outputs/descriptor");
        let desc = desc_str.parse().unwrap();
        let expected_dest = "tlq1qqwx9sng3htz6u2yeqrgf2w525att79vnvwtcqsar7xyqj8hf7s32usgvct9q9f4u3nmnnkwhkfayswc853egs7cvnfs3t7zty";
        let expected_asset_id = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
        let expected_value = 120;
        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID_TESTNET).unwrap();
        assert_eq!(balance.recipients().len(), 1);
        let recipient = balance.recipients().first().unwrap();
        let dest = recipient.address().unwrap();
        assert_eq!(dest.to_string(), expected_dest);
        assert_eq!(recipient.asset().unwrap().to_string(), expected_asset_id);
        assert_eq!(recipient.value().unwrap(), expected_value);
        assert_eq!(recipient.vout(), 0);

        let balance = pset_balance(&pset, &desc, &elements::AddressParams::LIQUID).unwrap();
        assert_eq!(balance.recipients().len(), 1);
        let recipient = balance.recipients().first().unwrap();
        let dest = recipient.address().unwrap();
        assert_ne!(dest.to_string(), expected_dest);
        assert_eq!(dest.to_string(), "lq1qqwx9sng3htz6u2yeqrgf2w525att79vnvwtcqsar7xyqj8hf7s32usgvct9q9f4u3nmnnkwhkfayswc853egsw4pnw8lktr6d");
        assert_eq!(recipient.asset().unwrap().to_string(), expected_asset_id);
        assert_eq!(recipient.value().unwrap(), expected_value);
        assert_eq!(recipient.vout(), 0);
    }

    #[test]
    fn test_pset_debug() {
        let pset_str = include_str!("../test_data/pset_outputs/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let debug = crate::pset_debug(&pset);
        let expected = include_str!("../test_data/pset_debug.txt");
        assert_eq!(normalize_newlines(&debug), normalize_newlines(expected));
    }
}
