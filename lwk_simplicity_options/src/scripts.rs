use simplicityhl::elements::{
    taproot, Address, AddressParams, AssetId, ContractHash, OutPoint, Script,
};

use simplicityhl::simplicity::bitcoin::{secp256k1, XOnlyPublicKey};
use simplicityhl::simplicity::hashes::{sha256, Hash};
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
#[must_use]
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

fn script_version(cmr: simplicityhl::simplicity::Cmr) -> (Script, taproot::LeafVersion) {
    let script = Script::from(cmr.as_ref().to_vec());
    (script, simplicityhl::simplicity::leaf_version())
}

fn taproot_spending_info(
    cmr: simplicityhl::simplicity::Cmr,
    internal_key: XOnlyPublicKey,
) -> taproot::TaprootSpendInfo {
    let builder = taproot::TaprootBuilder::new();
    let (script, version) = script_version(cmr);
    let builder = builder
        .add_leaf_with_ver(0, script, version)
        .expect("tap tree should be valid");
    builder
        .finalize(secp256k1::SECP256K1, internal_key)
        .expect("tap tree should be valid")
}

/// Compute the Taproot control block for script-path spending.
///
/// # Panics
///
/// Panics if the taproot tree is invalid (should never happen with valid CMR).
#[must_use]
pub fn control_block(
    cmr: simplicityhl::simplicity::Cmr,
    internal_key: XOnlyPublicKey,
) -> taproot::ControlBlock {
    let info = taproot_spending_info(cmr, internal_key);
    let script_ver = script_version(cmr);

    info.control_block(&script_ver)
        .expect("control block should exist")
}

/// SHA256 hash of an address's scriptPubKey bytes.
#[must_use]
pub fn hash_script(script: &Script) -> [u8; 32] {
    sha256::Hash::hash(script.as_bytes()).to_byte_array()
}

/// Compute issuance entropy for a new asset given an outpoint and contract hash entropy.
#[must_use]
pub fn get_new_asset_entropy(outpoint: &OutPoint, entropy: [u8; 32]) -> sha256::Midstate {
    let contract_hash = ContractHash::from_byte_array(entropy);
    AssetId::generate_asset_entropy(*outpoint, contract_hash)
}
