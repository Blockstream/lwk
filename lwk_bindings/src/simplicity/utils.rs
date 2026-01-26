use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity_options::simplicityhl;
use lwk_simplicity_options::utils::parse_genesis_hash;

use crate::blockdata::tx_out::TxOut;
use crate::types::{Hex, XOnlyPublicKey};
use crate::LwkError;

/// Get the x-only public key for a given derivation path from a signer.
#[uniffi::export]
pub fn simplicity_derive_xonly_pubkey(
    signer: &crate::Signer,
    derivation_path: String,
) -> Result<Arc<XOnlyPublicKey>, LwkError> {
    let keypair = derive_keypair(signer, &derivation_path)?;
    Ok(XOnlyPublicKey::from_keypair(&keypair))
}

pub(crate) fn get_genesis_hash(
    genesis_hash: &Hex,
) -> Result<simplicityhl::elements::BlockHash, LwkError> {
    parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
        msg: msg.to_string(),
    })
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
) -> Result<elements::bitcoin::secp256k1::Keypair, LwkError> {
    let path = DerivationPath::from_str(derivation_path).map_err(|e| LwkError::Generic {
        msg: format!("Invalid derivation path: {e}"),
    })?;

    let derived_xprv = signer.inner.derive_xprv(&path)?;
    Ok(elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derived_xprv.private_key,
    ))
}
