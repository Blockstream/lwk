use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::elements::schnorr::TweakedPublicKey;
use simplicityhl::elements::{taproot, Address, AddressParams, Script};

use simplicityhl::simplicity::bitcoin::{secp256k1, XOnlyPublicKey};
use simplicityhl::simplicity::hashes::{sha256, HashEngine};
use simplicityhl::{Arguments, CompiledProgram, UnstableFeatures};

use crate::error::ProgramError;

/// Load program source and compile it to a Simplicity program.
///
/// # Errors
/// Returns error if the program fails to compile.
pub fn load_program(source: &str, arguments: Arguments) -> Result<CompiledProgram, ProgramError> {
    load_program_with_debug_symbols(source, arguments, false)
}

/// Load program source and compile it to a Simplicity program, optionally including debug symbols.
///
/// Debug symbols can be included to provide more detailed error messages during program execution, at the cost of a larger program size.
/// Note that including debug symbols will change the CMR of the program, so it should only be used for testing and debugging purposes.
/// # Errors
/// Returns error if the program fails to compile.
pub fn load_program_with_debug_symbols(
    source: &str,
    arguments: Arguments,
    include_debug_symbols: bool,
) -> Result<CompiledProgram, ProgramError> {
    let compiled = CompiledProgram::new_with_unstable(
        source,
        &UnstableFeatures::all(),
        arguments,
        include_debug_symbols,
        Box::new(ElementsJetHinter::new()),
    )
    .map_err(ProgramError::Compilation)?;

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

/// Verify that a control block commits to the given CMR and UTXO script pubkey.
///
/// This checks that the control block belongs to the Simplicity leaf identified by
/// `cmr`, uses the Simplicity leaf version, and commits to the P2TR output key in
/// `script_pubkey`.
///
/// # Errors
///
/// Returns error if the script pubkey is not P2TR, if the leaf version is not the
/// Simplicity leaf version, or if the control block does not match the script pubkey.
pub fn verify_control_block(
    cmr: simplicityhl::simplicity::Cmr,
    control_block: &taproot::ControlBlock,
    script_pubkey: &Script,
) -> Result<(), ProgramError> {
    let (script, leaf_version) = script_version(cmr);

    if control_block.leaf_version != leaf_version {
        return Err(ProgramError::ControlBlockLeafVersionMismatch {
            expected: leaf_version.as_u8(),
            actual: control_block.leaf_version.as_u8(),
        });
    }

    if !script_pubkey.is_v1_p2tr() {
        return Err(ProgramError::ScriptPubkeyNotP2tr {
            actual_hash: script_pubkey.script_hash().to_string(),
        });
    }

    let output_key = XOnlyPublicKey::from_slice(&script_pubkey.as_bytes()[2..34])
        .map_err(|e| ProgramError::InvalidTaprootOutputKey(e.to_string()))?;
    let output_key = TweakedPublicKey::new(output_key);

    if !control_block.verify_taproot_commitment(secp256k1::SECP256K1, &output_key, &script) {
        return Err(ProgramError::ControlBlockMismatch {
            cmr: cmr.to_string(),
            script_pubkey: script_pubkey.to_string(),
        });
    }

    Ok(())
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

    use std::str::FromStr;

    #[test]
    fn test_tap_data_hash() {
        assert_eq!(
            tap_data_hash([0u8; 32].as_ref()).to_string(),
            "a33ad504fd45357a3909bf9dea8ce4aca38fe6e7d9c9d3e9e01211408990123f"
        )
    }

    #[test]
    fn test_verify_control_block() {
        let cmr = simplicityhl::simplicity::Cmr::from_str(
            "cbd8d3d0cc95384237c1bf20334c30b579f22058563c37731a3ab2bc76d5a248",
        )
        .unwrap();
        let internal_key = XOnlyPublicKey::from_str(
            "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
        )
        .unwrap();
        let other_internal_key = XOnlyPublicKey::from_str(
            "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083",
        )
        .unwrap();

        let (script, version) = script_version(cmr);
        let state_hash =
            taproot::TapNodeHash::from_byte_array(tap_data_hash(b"state").to_byte_array());

        let spend_info = taproot::TaprootBuilder::new()
            .add_leaf_with_ver(1, script.clone(), version)
            .unwrap()
            .add_hidden(1, state_hash)
            .unwrap()
            .finalize(secp256k1::SECP256K1, internal_key)
            .unwrap();
        let script_pubkey = Script::new_v1_p2tr_tweaked(spend_info.output_key());
        let control_block = spend_info
            .control_block(&(script.clone(), version))
            .unwrap();

        verify_control_block(cmr, &control_block, &script_pubkey).unwrap();

        let other_spend_info = taproot::TaprootBuilder::new()
            .add_leaf_with_ver(0, script.clone(), version)
            .unwrap()
            .finalize(secp256k1::SECP256K1, other_internal_key)
            .unwrap();
        let other_control_block = other_spend_info
            .control_block(&(script.clone(), version))
            .unwrap();

        assert!(matches!(
            verify_control_block(cmr, &other_control_block, &script_pubkey),
            Err(ProgramError::ControlBlockMismatch { .. })
        ));

        let mut wrong_version_control_block = control_block.clone();
        wrong_version_control_block.leaf_version = taproot::LeafVersion::TAPSCRIPT;
        assert!(matches!(
            verify_control_block(cmr, &wrong_version_control_block, &script_pubkey),
            Err(ProgramError::ControlBlockLeafVersionMismatch { .. })
        ));

        assert!(matches!(
            verify_control_block(cmr, &control_block, &Script::new()),
            Err(ProgramError::ScriptPubkeyNotP2tr { .. })
        ));
    }
}
