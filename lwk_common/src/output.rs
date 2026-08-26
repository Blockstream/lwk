use std::collections::BTreeMap;

use elements::Script;

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
