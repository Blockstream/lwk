// Code from rust-elements and the ledger bitcoin app
// TODO: check if it's actually necessary to copy all of these code

#[macro_use]
mod macros;
mod global;
mod input;
mod output;

use elements_miniscript::elements::{
    bitcoin::key::FromSliceError as KeyError,
    bitcoin::secp256k1::{self, XOnlyPublicKey},
    bitcoin::taproot::{self, TapLeafHash},
    bitcoin::{self, ecdsa, PublicKey},
    encode::{deserialize, serialize},
    hashes::Hash,
    pset::raw,
};
// get_pairs functions are not exposed
pub use global::get_v2_global_pairs;
pub use input::get_v2_input_pairs;
pub use output::get_v2_output_pairs;

pub fn deserialize_pair(pair: raw::Pair) -> (Vec<u8>, Vec<u8>) {
    (deserialize(&serialize(&pair.key)).unwrap(), pair.value)
}

#[allow(unused)]
pub enum PartialSignature {
    /// signature stored in pbst.partial_sigs
    Sig(PublicKey, ecdsa::Signature),
    /// signature stored in pbst.tap_script_sigs
    TapScriptSig(XOnlyPublicKey, Option<TapLeafHash>, taproot::Signature),
}

impl PartialSignature {
    #[allow(unused)]
    pub fn from_slice(slice: &[u8]) -> Result<Self, PartialSignatureError> {
        let key_augment_byte = slice
            .first()
            .ok_or(PartialSignatureError::BadKeyAugmentLength)?;
        let key_augment_len = u8::from_le_bytes([*key_augment_byte]) as usize;

        if key_augment_len >= slice.len() {
            Err(PartialSignatureError::BadKeyAugmentLength)
        } else if key_augment_len == 64 {
            let key = XOnlyPublicKey::from_slice(&slice[1..33])
                .map_err(PartialSignatureError::XOnlyPubKey)?;
            let tap_leaf_hash =
                TapLeafHash::from_slice(&slice[33..65]).map_err(PartialSignatureError::TapLeaf)?;
            let sig = taproot::Signature::from_slice(&slice[65..])
                .map_err(PartialSignatureError::TaprootSig)?;
            Ok(Self::TapScriptSig(key, Some(tap_leaf_hash), sig))
        } else if key_augment_len == 32 {
            let key = XOnlyPublicKey::from_slice(&slice[1..33])
                .map_err(PartialSignatureError::XOnlyPubKey)?;
            let sig = taproot::Signature::from_slice(&slice[65..])
                .map_err(PartialSignatureError::TaprootSig)?;
            Ok(Self::TapScriptSig(key, None, sig))
        } else {
            let key = PublicKey::from_slice(&slice[1..key_augment_len + 1])
                .map_err(PartialSignatureError::PubKey)?;
            let sig = ecdsa::Signature::from_slice(&slice[key_augment_len + 1..])
                .map_err(PartialSignatureError::EcdsaSig)?;
            Ok(Self::Sig(key, sig))
        }
    }
}

#[allow(unused)]
pub enum PartialSignatureError {
    BadKeyAugmentLength,
    XOnlyPubKey(secp256k1::Error),
    PubKey(KeyError),
    EcdsaSig(ecdsa::Error),
    TaprootSig(taproot::SigFromSliceError),
    TapLeaf(bitcoin::hashes::FromSliceError),
}
