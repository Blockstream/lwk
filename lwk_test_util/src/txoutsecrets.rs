use elements_miniscript::elements::{self};

use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::TxOutSecrets;
use elements::Txid;
use std::str::FromStr;

fn asset_blinding_factor_test_vector() -> AssetBlindingFactor {
    "0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap()
}

fn value_blinding_factor_test_vector() -> ValueBlindingFactor {
    "0000000000000000000000000000000000000000000000000000000000000002"
        .parse()
        .unwrap()
}

pub fn txid_test_vector() -> Txid {
    Txid::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()
}

pub fn tx_out_secrets_test_vector() -> TxOutSecrets {
    elements::TxOutSecrets::new(
        crate::asset::regtest_policy_asset(),
        asset_blinding_factor_test_vector(),
        1000,
        value_blinding_factor_test_vector(),
    )
}

pub fn tx_out_secrets_test_vector_bytes() -> Vec<u8> {
    elements::hex::decode_to_vec(include_str!("../test_data/tx_out_secrets_test_vector.hex"))
        .unwrap()
}
