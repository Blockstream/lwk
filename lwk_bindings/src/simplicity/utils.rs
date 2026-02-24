use crate::blockdata::tx_out::TxOut;
use crate::types::XOnlyPublicKey;
use crate::{ControlBlock, LwkError};

use super::cmr::Cmr;

use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::DerivationPath;

use lwk_simplicity::scripts;
use lwk_wollet::{secp256k1::Keypair, EC};

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
    cmr: &Cmr,
    internal_key: &XOnlyPublicKey,
) -> Result<Arc<ControlBlock>, LwkError> {
    let internal_key = internal_key.to_simplicityhl()?;
    let control_block = scripts::control_block(cmr.inner(), internal_key);
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
    use crate::types::Hex;

    const TEST_PUBLIC_KEY: &str =
        "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083";
    const P2PK_SOURCE: &str = include_str!("../../../lwk_simplicity/data/p2pk.simf");

    #[test]
    fn test_control_block_roundtrip() {
        let args = SimplicityArguments::new().add_value(
            "PUBLIC_KEY".to_string(),
            &SimplicityTypedValue::u256(Hex::from_str(TEST_PUBLIC_KEY).unwrap()).unwrap(),
        );
        let program = SimplicityProgram::load(P2PK_SOURCE.to_string(), &args).unwrap();
        let cmr = program.cmr();

        let internal_key = XOnlyPublicKey::new(TEST_PUBLIC_KEY).unwrap();
        let control_block = simplicity_control_block(cmr.as_ref(), &internal_key).unwrap();
        let control_block_from_program = program.control_block(&internal_key).unwrap();

        assert_eq!(
            control_block_from_program,
            Hex::from(control_block.serialize())
        );
    }
}
