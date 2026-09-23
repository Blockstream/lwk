use elements_miniscript::elements::{self};

use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::hex::FromHex;
use elements::TxOutSecrets;
use elements::{AssetId, Txid};
use std::str::FromStr;

mod amp2;
mod auth;
mod block;
mod desc;
mod fee;
mod generate;
mod http;
mod lightningd;
mod mnemonic;
mod panic_store;
mod pegin;
mod pset;
mod registry;
mod test_env;
mod update;
mod waterfalls;
pub use auth::{
    AuthStack, AUTH_CLIENT_ID, AUTH_CLIENT_SECRET, AUTH_REALM, AUTH_SHORT_CLIENT_ID,
    AUTH_SHORT_CLIENT_SECRET, AUTH_USER_UUID,
};
pub use block::{liquid_block_1, liquid_block_header_2_963_520};
pub use desc::{
    add_checksum, wollet_descriptor_many_transactions, wollet_descriptor_string,
    wollet_descriptor_string2, TEST_DESCRIPTOR,
};
pub use fee::{assert_fee_rate, compute_fee_rate, compute_fee_rate_without_discount_ct};
pub use generate::{generate_mnemonic, generate_slip77, generate_view_key, generate_xprv};
pub use http::serve_http_response;
pub use mnemonic::{
    DEFAULT_SPECULOS_MNEMONIC, TEST_MNEMONIC, TEST_MNEMONIC_SLIP77, TEST_MNEMONIC_XPUB,
};
pub use panic_store::PanicStore;
pub use pegin::{
    FED_PEG_DESC, FED_PEG_SCRIPT, FED_PEG_SCRIPT_ASM, PEGIN_TEST_ADDR, PEGIN_TEST_DESC,
};
pub use pset::{
    descriptor_pset_usdt_no_contracts, n_issuances, n_reissuances, pset_rt, pset_usdt_no_contracts,
    pset_usdt_with_contract, psets_to_combine,
};
pub use test_env::{TestEnv, TestEnvBuilder};
pub use update::{
    update_merge_test_1, update_merge_test_2, update_merge_test_3, update_merge_test_descriptor,
    update_test_vector_2_bytes, update_test_vector_bytes, update_test_vector_encrypted_base64,
    update_test_vector_encrypted_bytes, update_test_vector_encrypted_bytes2,
    update_test_vector_many_transactions, update_test_vector_v1_bytes, update_test_vector_v4_bytes,
    update_v2_test_vector_after_many_transactions,
};

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
