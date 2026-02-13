use simplicityhl::elements::{taproot, Address, AddressParams, Script};

use simplicityhl::simplicity::bitcoin::{secp256k1, XOnlyPublicKey};
use simplicityhl::simplicity::hashes::{sha256, Hash, HashEngine};
use simplicityhl::{Arguments, CompiledProgram};

use crate::error::ProgramError;

/// Load program source and compile it to a Simplicity program.
///
/// # Errors
/// Returns error if the program fails to compile.
pub fn load_program(source: &str, arguments: Arguments) -> Result<CompiledProgram, ProgramError> {
    let compiled =
        CompiledProgram::new(source, arguments, true).map_err(ProgramError::Compilation)?;

    Ok(compiled)
}

/// Generate a non-confidential P2TR address for the given program CMR and key.
pub fn create_p2tr_address(
    cmr: simplicityhl::simplicity::Cmr,
    x_only_public_key: &XOnlyPublicKey,
    params: &'static AddressParams,
) -> Address {
    let spend_info = taproot_spending_info(cmr, *x_only_public_key);

    Address::p2tr(
        secp256k1::SECP256K1,
        spend_info.internal_key(),
        spend_info.merkle_root(),
        None,
        params,
    )
}

/// Return the version of Simplicity leaves inside a tap tree.
pub fn simplicity_leaf_version() -> taproot::LeafVersion {
    simplicityhl::simplicity::leaf_version()
}

/// Create a SHA256 context, initialized with a "TapData" tag and data
///
/// Based on the C implementation of the `tapdata_init` jet:
/// https://github.com/BlockstreamResearch/simplicity/blob/d190505509f4c04b1b9193c6739515f9faa18aac/C/jets.c#L1408
pub fn tap_data_hash(data: &[u8]) -> sha256::Hash {
    let tag = sha256::Hash::hash(b"TapData");
    let mut eng = sha256::Hash::engine();
    eng.input(tag.as_byte_array());
    eng.input(tag.as_byte_array());
    eng.input(data);
    sha256::Hash::from_engine(eng)
}

/// Compute the Taproot control block for script-path spending.
///
/// # Panics
///
/// Panics if the taproot tree is invalid (should never happen with valid CMR).
pub fn control_block(
    cmr: simplicityhl::simplicity::Cmr,
    internal_key: XOnlyPublicKey,
) -> taproot::ControlBlock {
    let info = taproot_spending_info(cmr, internal_key);
    let script_ver = script_version(cmr);

    info.control_block(&script_ver)
        .expect("control block should exist")
}

/// Returns pair (Script, LeafVersion) for the CMR of Simplicity program
fn script_version(cmr: simplicityhl::simplicity::Cmr) -> (Script, taproot::LeafVersion) {
    let script = Script::from(cmr.as_ref().to_vec());
    (script, simplicity_leaf_version())
}

fn taproot_spending_info(
    cmr: simplicityhl::simplicity::Cmr,
    internal_key: XOnlyPublicKey,
) -> taproot::TaprootSpendInfo {
    let (script, version) = script_version(cmr);
    let builder = taproot::TaprootBuilder::new()
        .add_leaf_with_ver(0, script, version)
        .expect("tap tree should be valid");
    builder
        .finalize(secp256k1::SECP256K1, internal_key)
        .expect("tap tree should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tap_data_hash() {
        assert_eq!(
            tap_data_hash([0u8; 32].as_ref()).to_string(),
            "a33ad504fd45357a3909bf9dea8ce4aca38fe6e7d9c9d3e9e01211408990123f"
        )
    }
}
