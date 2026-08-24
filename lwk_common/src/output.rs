use std::collections::BTreeMap;

use elements::pset::{Output, PartiallySignedTransaction};
use elements::secp256k1_zkp::{All, Secp256k1};
use elements::{AssetId, BlindAssetProofs, BlindValueProofs, Script};

use elements::bitcoin::bip32::{DerivationPath, Fingerprint};
use elements::bitcoin::PublicKey as BitcoinPublicKey;
use elements::taproot::TapLeafHash;
use elements_miniscript::elements::secp256k1_zkp::XOnlyPublicKey;

use elements_miniscript::{
    ConfidentialDescriptor, DescriptorPublicKey as MiniscriptDescriptorPublicKey,
};

use crate::Error;

/// Return whether an output belongs to the given descriptor, along with its derivation path.
pub(crate) fn is_mine(
    script_pubkey: &Script,
    descriptor: &ConfidentialDescriptor<MiniscriptDescriptorPublicKey>,
    bip32_derivation: &BTreeMap<BitcoinPublicKey, (Fingerprint, DerivationPath)>,
    tap_key_origins: &BTreeMap<XOnlyPublicKey, (Vec<TapLeafHash>, (Fingerprint, DerivationPath))>,
) -> Result<(bool, Option<DerivationPath>), Error> {
    let paths: Vec<&DerivationPath> = if script_pubkey.is_v1_p2tr() {
        tap_key_origins
            .values()
            .map(|(_, (_, path))| path)
            .collect()
    } else {
        bip32_derivation.values().map(|(_, path)| path).collect()
    };

    // Without a wildcard the derivation index is irrelevant: every index derives the same
    // script for a given (possibly multi-path) single descriptor. So we can check for a match
    // directly, without relying on any derivation info from the PSET.
    if !descriptor.descriptor.has_wildcard() {
        for d in descriptor.descriptor.clone().into_single_descriptors()? {
            let mine = d.at_derivation_index(0)?.script_pubkey();
            if &mine == script_pubkey {
                return Ok((true, paths.first().cloned().cloned()));
            }
        }
        return Ok((false, None));
    }

    for path in paths {
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
                return Ok((true, Some(path.clone())));
            }
        }
    }
    Ok((false, None))
}

/// The details of an output of a PSET
#[derive(Debug, Clone)]
pub struct OutputDetails {
    asset: Option<AssetId>,
    value: Option<u64>,
    is_fully_explicit: bool,
    is_fully_confidential: bool,
    vout: u32,
    script_pubkey: Script,
    is_owned: bool,
    derivation_path: Option<DerivationPath>,
}

impl OutputDetails {
    /// Create the details of a PSET output
    ///
    /// The asset and amount are always verified against the commitments when the output is
    /// blinded, using the blind proofs present in the PSET. If a value cannot be verified it is
    /// reported as `None`.
    pub(crate) fn new(
        secp: &Secp256k1<All>,
        output: &Output,
        vout: u32,
        is_owned: bool,
        derivation_path: Option<DerivationPath>,
    ) -> Result<Self, Error> {
        let script_pubkey = output.script_pubkey.clone();
        let (asset, value) = verified_asset_value(secp, output, vout as usize)?;
        let is_fully_explicit = output_is_fully_explicit(output);
        let is_fully_confidential = output.asset_comm.is_some() && output.amount_comm.is_some();
        Ok(Self {
            asset,
            value,
            is_fully_explicit,
            is_fully_confidential,
            vout,
            script_pubkey,
            is_owned,
            derivation_path,
        })
    }

    /// The asset of the output, or None if it couldn't be verified against the commitments
    pub fn asset(&self) -> Option<AssetId> {
        self.asset
    }

    /// The amount of the output in satoshis, or None if it couldn't be verified against
    /// the commitments
    pub fn satoshi(&self) -> Option<u64> {
        self.value
    }

    /// Whether the output is fully explicit, i.e., both the asset and the amount are explicit
    /// with no commitments and no other blinding field
    pub fn is_fully_explicit(&self) -> bool {
        self.is_fully_explicit
    }

    /// Whether the output is fully confidential
    pub fn is_fully_confidential(&self) -> bool {
        self.is_fully_confidential
    }

    /// Whether this is the fee output
    pub fn is_fee(&self) -> bool {
        self.script_pubkey.is_empty()
    }

    /// The script pubkey of the output
    pub fn script_pubkey(&self) -> &Script {
        &self.script_pubkey
    }

    /// The index of the output in the transaction
    pub fn vout(&self) -> u32 {
        self.vout
    }

    /// The derivation path of the output, if it belongs to the wallet
    pub fn derivation_path(&self) -> Option<&DerivationPath> {
        self.derivation_path.as_ref()
    }

    /// Whether the output belongs to the wallet
    pub fn is_owned(&self) -> bool {
        self.is_owned
    }
}

/// Return true if the output is fully explicit, i.e., both the asset and the amount are explicit
/// and none of the blinding fields are present.
fn output_is_fully_explicit(output: &Output) -> bool {
    output.asset.is_some()
        && output.amount.is_some()
        && output.asset_comm.is_none()
        && output.amount_comm.is_none()
        && output.blinding_key.is_none()
        && output.ecdh_pubkey.is_none()
        && output.value_rangeproof.is_none()
        && output.asset_surjection_proof.is_none()
        && output.blind_value_proof.is_none()
        && output.blind_asset_proof.is_none()
}

/// Return the asset and amount of an output, always verified against the commitments when the
/// output is blinded.
///
/// The values are reported as `None` when they cannot be verified, e.g., for a blinded output
/// without blind proofs in the PSET.
fn verified_asset_value(
    secp: &Secp256k1<All>,
    output: &Output,
    idx: usize,
) -> Result<(Option<AssetId>, Option<u64>), Error> {
    if output_is_fully_explicit(output) {
        return Ok((output.asset, output.amount));
    } else if let (
        Some(asset_comm),
        Some(amount_comm),
        Some(asset),
        Some(amount),
        Some(blind_asset_proof),
        Some(blind_value_proof),
    ) = (
        output.asset_comm,
        output.amount_comm,
        output.asset,
        output.amount,
        output.blind_asset_proof.as_ref(),
        output.blind_value_proof.as_ref(),
    ) {
        if !blind_asset_proof.blind_asset_proof_verify(secp, asset, asset_comm) {
            return Err(Error::InvalidAssetBlindProof { idx });
        }
        if !blind_value_proof.blind_value_proof_verify(secp, amount, asset_comm, amount_comm) {
            return Err(Error::InvalidValueBlindProof { idx });
        }
        return Ok((Some(asset), Some(amount)));
    };

    Ok((None, None))
}

/// Return the details of the outputs of a PSET.
///
/// The asset and amount of each output are always verified against the commitments when the output
/// is blinded, using the blind proofs present in the PSET.
pub(crate) fn pset_outputs_details(
    pset: &PartiallySignedTransaction,
    descriptor: &ConfidentialDescriptor<MiniscriptDescriptorPublicKey>,
) -> Result<Vec<OutputDetails>, Error> {
    let secp = Secp256k1::new();
    pset.outputs()
        .iter()
        .enumerate()
        .map(|(idx, output)| {
            let (is_mine, derivation_path) = is_mine(
                &output.script_pubkey,
                descriptor,
                &output.bip32_derivation,
                &output.tap_key_origins,
            )?;
            OutputDetails::new(&secp, output, idx as u32, is_mine, derivation_path)
        })
        .collect()
}
