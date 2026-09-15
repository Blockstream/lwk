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
                // TODO: improve by selecting path with matching pubkey
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
pub(crate) fn verified_asset_value(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pset_outputs_details() {
        let pset_str = include_str!("../test_data/pset_outputs/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let desc_str = include_str!("../test_data/pset_outputs/descriptor");
        let desc: ConfidentialDescriptor<MiniscriptDescriptorPublicKey> = desc_str.parse().unwrap();
        let outputs = pset_outputs_details(&pset, &desc).unwrap();
        let testnet_asset = *crate::Network::TestnetLiquid.policy_asset();

        assert_eq!(outputs.len(), 3);

        // external recipient
        let recipient = &outputs[0];
        assert_eq!(recipient.vout(), 0);
        assert!(!recipient.is_owned());
        assert!(!recipient.is_fee());
        assert!(recipient.is_fully_confidential());
        assert!(!recipient.is_fully_explicit());
        assert!(recipient.derivation_path().is_none());
        assert_eq!(recipient.asset().unwrap(), testnet_asset);
        assert_eq!(recipient.satoshi(), Some(120));

        // change output
        let change = &outputs[1];
        assert_eq!(change.vout(), 1);
        assert!(change.is_owned());
        assert!(!change.is_fee());
        assert!(change.is_fully_confidential());
        assert_eq!(
            change.derivation_path().unwrap().to_string(),
            "84'/1'/0'/1/4"
        );
        assert_eq!(change.satoshi(), Some(88643));

        // fee output
        let fee = &outputs[2];
        assert_eq!(fee.vout(), 2);
        assert!(fee.is_fee());
        assert!(fee.is_fully_explicit());
        assert!(!fee.is_fully_confidential());
        assert!(!fee.is_owned());
        assert!(fee.derivation_path().is_none());
        assert_eq!(fee.asset().unwrap(), testnet_asset);
        assert_eq!(fee.satoshi(), Some(26));

        // PsetDetails::outputs() is filled in as well
        let details =
            crate::model::PsetDetails::new(&pset, &desc, &crate::Network::TestnetLiquid).unwrap();
        let outputs = details.outputs();
        assert_eq!(outputs.len(), 3);
        assert!(!outputs[0].is_owned());
        assert!(outputs[1].is_owned());
        assert!(outputs[2].is_fee());

        // without blind proofs the blinded change output cannot be verified
        let mut pset_no_proofs = pset.clone();
        let change = &mut pset_no_proofs.outputs_mut()[1];
        assert!(change.blind_asset_proof.is_some());
        assert!(change.blind_value_proof.is_some());
        change.blind_asset_proof = None;
        change.blind_value_proof = None;

        let outputs = pset_outputs_details(&pset_no_proofs, &desc).unwrap();
        let change = &outputs[1];
        assert!(change.is_owned());
        assert_eq!(change.asset(), None);
        assert_eq!(change.satoshi(), None);

        // corrupt the asset of the external recipient so the asset blind proof fails
        let mut pset_invalid_asset = pset.clone();

        pset_invalid_asset.outputs_mut()[0].asset = Some(*crate::Network::Liquid.policy_asset());

        let err = pset_outputs_details(&pset_invalid_asset, &desc).unwrap_err();
        assert!(matches!(
            err,
            crate::Error::InvalidAssetBlindProof { idx: 0 }
        ));

        // corrupt the amount of the external recipient so the value blind proof fails
        // while the asset blind proof still verifies
        let mut pset_invalid_value = pset.clone();
        pset_invalid_value.outputs_mut()[0].amount = Some(9999);

        let err = pset_outputs_details(&pset_invalid_value, &desc).unwrap_err();
        assert!(matches!(
            err,
            crate::Error::InvalidValueBlindProof { idx: 0 }
        ));

        // make the owned change output only partially blinded by removing the amount
        // commitment, so that neither the asset nor the amount can be verified
        let mut pset_partially_blinded = pset.clone();
        pset_partially_blinded.outputs_mut()[1].amount_comm = None;

        let outputs = pset_outputs_details(&pset_partially_blinded, &desc).unwrap();
        let change = &outputs[1];
        assert!(change.is_owned());
        assert_eq!(change.asset(), None);
        assert_eq!(change.satoshi(), None);
        assert!(!change.is_fully_confidential());
        assert!(!change.is_fully_explicit());

        // an explicit output with a blinding field present
        let mut pset_explicit_with_blinding_key = pset.clone();
        let secp = elements::secp256k1_zkp::Secp256k1::new();
        let secret = elements::secp256k1_zkp::SecretKey::from_slice(&[0x01u8; 32]).unwrap();
        pset_explicit_with_blinding_key.outputs_mut()[2].blinding_key =
            Some(elements::bitcoin::PublicKey::new(
                elements::secp256k1_zkp::PublicKey::from_secret_key(&secp, &secret),
            ));

        let outputs = pset_outputs_details(&pset_explicit_with_blinding_key, &desc).unwrap();
        let fee = &outputs[2];
        assert!(!fee.is_fully_explicit());
        assert_eq!(fee.asset(), None);
        assert_eq!(fee.satoshi(), None);
    }
}
