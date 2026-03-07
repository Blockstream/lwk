use crate::blockdata::tx_out::TxOut;
use crate::types::{PublicKey, XOnlyPublicKey};
use crate::{ControlBlock, LwkError, Pset};

use super::cmr::Cmr;

use std::str::FromStr;
use std::sync::Arc;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};
use lwk_wollet::hashes::hex::FromHex;

use lwk_simplicity::scripts;
use lwk_wollet::secp256k1::{Keypair, Message};
use lwk_wollet::EC;

/// Get the x-only public key for a given derivation path from a signer.
#[uniffi::export]
pub fn simplicity_derive_xonly_pubkey(
    signer: &crate::Signer,
    derivation_path: &str,
) -> Result<Arc<XOnlyPublicKey>, LwkError> {
    let keypair = derive_keypair(signer, derivation_path)?;
    Ok(XOnlyPublicKey::from_keypair(&keypair))
}

/// Derive a compressed public key for a given derivation path from a signer.
#[uniffi::export]
pub fn simplicity_derive_pubkey(
    signer: &crate::Signer,
    derivation_path: &str,
) -> Result<Arc<PublicKey>, LwkError> {
    let keypair = derive_keypair(signer, derivation_path)?;
    let pubkey = keypair.public_key();
    Ok(Arc::new(elements::bitcoin::PublicKey::new(pubkey).into()))
}

/// Sign a 32-byte message digest with Schnorr signature using a derived key.
#[uniffi::export]
pub fn simplicity_sign_schnorr(
    signer: &crate::Signer,
    derivation_path: &str,
    message: Vec<u8>,
) -> Result<Vec<u8>, LwkError> {
    let keypair = derive_keypair(signer, derivation_path)?;
    let msg = Message::from_digest_slice(&message).map_err(|error| LwkError::Generic {
        msg: format!("invalid schnorr message digest: {error}"),
    })?;
    Ok(keypair.sign_schnorr(msg).serialize().to_vec())
}

/// Derive a compressed public key from a base58check xpub.
#[uniffi::export]
pub fn simplicity_pubkey_from_xpub(xpub: String) -> Result<Arc<PublicKey>, LwkError> {
    let xpub = Xpub::from_str(xpub.trim()).map_err(|error| LwkError::Generic {
        msg: format!("invalid xpub: {error}"),
    })?;
    Ok(Arc::new(
        elements::bitcoin::PublicKey::new(xpub.public_key).into(),
    ))
}

/// Apply externally produced signatures into a PSET by signer fingerprint.
///
/// One signature is consumed per matched input in input order.
#[uniffi::export]
pub fn simplicity_apply_signatures_to_pset(
    pst: Arc<Pset>,
    signer_fingerprint_hex: String,
    signatures: Vec<String>,
) -> Result<Arc<Pset>, LwkError> {
    let signer_fingerprint = parse_signer_fingerprint(&signer_fingerprint_hex)?;
    let mut inner = pst.inner();
    let mut inputs_to_sign = Vec::new();

    for (input_index, input) in inner.inputs().iter().enumerate() {
        let mut matching_pubkeys = input
            .bip32_derivation
            .iter()
            .filter_map(|(pubkey, (fp, _))| {
                if *fp == signer_fingerprint {
                    Some(*pubkey)
                } else {
                    None
                }
            });

        let pubkey = matching_pubkeys.next();
        if matching_pubkeys.next().is_some() {
            return Err(LwkError::Generic {
                msg: format!(
                    "input {input_index} has multiple derivations for fingerprint {signer_fingerprint}"
                ),
            });
        }

        if let Some(pubkey) = pubkey {
            inputs_to_sign.push((input_index, pubkey));
        }
    }

    if inputs_to_sign.is_empty() {
        return Err(LwkError::Generic {
            msg: format!("no pset inputs matched signer fingerprint {signer_fingerprint}"),
        });
    }

    if inputs_to_sign.len() != signatures.len() {
        return Err(LwkError::Generic {
            msg: format!(
                "signature count mismatch: expected {}, got {}",
                inputs_to_sign.len(),
                signatures.len()
            ),
        });
    }

    for ((input_index, pubkey), signature_str) in inputs_to_sign.into_iter().zip(signatures) {
        let signature =
            decode_signature_string(&signature_str).map_err(|msg| LwkError::Generic { msg })?;
        inner.inputs_mut()[input_index]
            .partial_sigs
            .insert(pubkey, signature);
    }

    Ok(Arc::new(Pset::from(inner)))
}

fn parse_signer_fingerprint(fingerprint_hex: &str) -> Result<Fingerprint, LwkError> {
    let normalized = fingerprint_hex
        .trim()
        .strip_prefix("0x")
        .unwrap_or(fingerprint_hex.trim());

    Fingerprint::from_str(normalized).map_err(|error| LwkError::Generic {
        msg: format!("invalid signer fingerprint '{normalized}': {error}"),
    })
}

fn decode_signature_string(signature: &str) -> Result<Vec<u8>, String> {
    let trimmed = signature.trim();
    if trimmed.is_empty() {
        return Err("signature string must not be empty".to_string());
    }

    if let Ok(bytes) = Vec::from_hex(trimmed) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    for decoder in [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD] {
        if let Ok(bytes) = decoder.decode(trimmed.as_bytes()) {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
        }
    }

    Err("signature must be non-empty hex or base64".to_string())
}

/// Compute the Taproot control block for Simplicity script-path spending.
#[uniffi::export]
pub fn simplicity_control_block(
    cmr: &Cmr,
    internal_key: &XOnlyPublicKey,
) -> Result<Arc<ControlBlock>, LwkError> {
    let internal_key = internal_key.to_simplicityhl()?;
    let control_block = scripts::control_block(cmr.inner(), internal_key);
    let serialized = control_block.serialize();
    ControlBlock::from_bytes(&serialized)
}

pub(crate) fn convert_utxos(utxos: &[Arc<TxOut>]) -> Vec<elements::TxOut> {
    utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect()
}

pub(crate) fn derive_keypair(
    signer: &crate::Signer,
    derivation_path: &str,
) -> Result<Keypair, LwkError> {
    let derived_xprv = signer
        .inner
        .derive_xprv(&DerivationPath::from_str(derivation_path)?)?;
    Ok(Keypair::from_secret_key(&EC, &derived_xprv.private_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::simplicity::{SimplicityArguments, SimplicityProgram, SimplicityTypedValue};

    use lwk_wollet::hashes::hex::FromHex;

    const TEST_PUBLIC_KEY: &str =
        "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083";
    const P2PK_SOURCE: &str = include_str!("../../../lwk_simplicity/data/p2pk.simf");

    #[test]
    fn test_control_block_roundtrip() {
        let args = SimplicityArguments::new().add_value(
            "PUBLIC_KEY".to_string(),
            &SimplicityTypedValue::u256(&Vec::<u8>::from_hex(TEST_PUBLIC_KEY).unwrap()).unwrap(),
        );
        let program = SimplicityProgram::load(P2PK_SOURCE, &args).unwrap();
        let cmr = program.cmr();

        let internal_key = XOnlyPublicKey::from_string(TEST_PUBLIC_KEY).unwrap();
        let control_block = simplicity_control_block(cmr.as_ref(), &internal_key).unwrap();
        let control_block_from_program = program.control_block(&internal_key).unwrap();

        assert_eq!(
            control_block_from_program.to_bytes(),
            control_block.to_bytes()
        );
    }
}
