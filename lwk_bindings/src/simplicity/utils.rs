use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity::scripts;
use lwk_simplicity::simplicityhl;

use crate::blockdata::tx_out::TxOut;
use crate::types::{Hex, XOnlyPublicKey};
use crate::{ControlBlock, LwkError};

/// Get the x-only public key for a given derivation path from a signer.
#[uniffi::export]
pub fn simplicity_derive_xonly_pubkey(
    signer: &crate::Signer,
    derivation_path: String,
) -> Result<Arc<XOnlyPublicKey>, LwkError> {
    let keypair = derive_keypair(signer, &derivation_path)?;
    Ok(XOnlyPublicKey::from_keypair(&keypair))
}

/// Compute the Taproot control block for Simplicity script-path spending.
#[uniffi::export]
pub fn simplicity_control_block(
    cmr: &Hex,
    internal_key: &XOnlyPublicKey,
) -> Result<Arc<ControlBlock>, LwkError> {
    let cmr = simplicityhl::simplicity::Cmr::from_byte_array(cmr.as_ref().try_into()?);
    let internal_key = internal_key.to_simplicityhl()?;
    let control_block = scripts::control_block(cmr, internal_key);
    let serialized = control_block.serialize();
    ControlBlock::from_slice(&serialized)
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
