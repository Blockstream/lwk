use elements_miniscript::elements::{self, BlockHeader};

use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::Decodable;
use elements::hex::FromHex;
use elements::{AssetId, Txid};
use elements::{Block, TxOutSecrets};
use std::str::FromStr;

mod amp2;
mod auth;
mod desc;
mod fee;
mod generate;
mod http;
mod lightningd;
mod panic_store;
mod pegin;
mod pset;
mod registry;
mod test_env;
mod waterfalls;
pub use auth::{
    AuthStack, AUTH_CLIENT_ID, AUTH_CLIENT_SECRET, AUTH_REALM, AUTH_SHORT_CLIENT_ID,
    AUTH_SHORT_CLIENT_SECRET, AUTH_USER_UUID,
};
pub use desc::{
    add_checksum, wollet_descriptor_many_transactions, wollet_descriptor_string,
    wollet_descriptor_string2, TEST_DESCRIPTOR,
};
pub use fee::{assert_fee_rate, compute_fee_rate, compute_fee_rate_without_discount_ct};
pub use generate::{generate_mnemonic, generate_slip77, generate_view_key, generate_xprv};
pub use http::serve_http_response;
pub use panic_store::PanicStore;
pub use pegin::{
    FED_PEG_DESC, FED_PEG_SCRIPT, FED_PEG_SCRIPT_ASM, PEGIN_TEST_ADDR, PEGIN_TEST_DESC,
};
pub use pset::{
    descriptor_pset_usdt_no_contracts, n_issuances, n_reissuances, pset_rt, pset_usdt_no_contracts,
    pset_usdt_with_contract, psets_to_combine,
};
pub use test_env::{TestEnv, TestEnvBuilder};

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
pub const TEST_MNEMONIC_XPUB: &str =
"tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s";
pub const TEST_MNEMONIC_SLIP77: &str =
    "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

pub const DEFAULT_SPECULOS_MNEMONIC: &str = "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin";

pub fn liquid_block_1() -> Block {
    let raw = include_bytes!(
        "../test_data/afafbbdfc52a45e51a3b634f391f952f6bdfd14ef74b34925954b4e20d0ad639.raw"
    );
    Block::consensus_decode(&raw[..]).unwrap()
}

pub fn liquid_block_header_2_963_520() -> BlockHeader {
    let hex = include_str!("../test_data/block_header_2_963_520.hex");
    let bytes = Vec::<u8>::from_hex(hex).unwrap();
    BlockHeader::consensus_decode(&bytes[..]).unwrap()
}

pub fn regtest_policy_asset() -> AssetId {
    AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225").unwrap()
}

pub fn init_logging() {
    let _ = env_logger::try_init();
}

fn asset_blinding_factor_test_vector() -> AssetBlindingFactor {
    AssetBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )
    .unwrap()
}

fn value_blinding_factor_test_vector() -> ValueBlindingFactor {
    ValueBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )
    .unwrap()
}

pub fn txid_test_vector() -> Txid {
    Txid::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()
}

pub fn tx_out_secrets_test_vector() -> TxOutSecrets {
    elements::TxOutSecrets::new(
        regtest_policy_asset(),
        asset_blinding_factor_test_vector(),
        1000,
        value_blinding_factor_test_vector(),
    )
}

pub fn tx_out_secrets_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/tx_out_secrets_test_vector.hex")).unwrap()
}

pub fn update_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
}

pub fn update_test_vector_v1_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v1.hex")).unwrap()
}

pub fn update_test_vector_v4_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v4.hex").trim()).unwrap()
}

pub fn update_test_vector_2_bytes() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector.bin").to_vec()
}

/// An update (serialized v1) with 63 transactions on liquid testnet wallet defined by [`wollet_descriptor_many_transactions`]
pub fn update_test_vector_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_many_txs.bin").to_vec()
}

/// An update (serialized v2) after [`update_test_vector_many_transactions`]
pub fn update_v2_test_vector_after_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_v2_after_many_txs.bin").to_vec()
}

/// First of 3 consecutive updates for merge testing
/// Contains initial wallet sync (tip only)
pub fn update_merge_test_1() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_1.bin").to_vec()
}

/// Second of 3 consecutive updates for merge testing
/// Contains wallet funding transaction
pub fn update_merge_test_2() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_2.bin").to_vec()
}

/// Third of 3 consecutive updates for merge testing
/// Contains spending transaction
pub fn update_merge_test_3() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_3.bin").to_vec()
}

/// Descriptor used for the merge test updates
pub fn update_merge_test_descriptor() -> String {
    include_str!("../test_data/merge_updates/descriptor.txt").to_string()
}

pub fn update_test_vector_encrypted_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!(
        "../test_data/update_test_vector_encrypted.hex"
    ))
    .unwrap()
}

pub fn update_test_vector_encrypted_base64() -> String {
    include_str!("../test_data/update_test_vector/update.base64").to_string()
}

pub fn update_test_vector_encrypted_bytes2() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector/000000000000").to_vec()
}
