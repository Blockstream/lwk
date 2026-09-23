use elements_miniscript::elements::{self, BlockHeader};

use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::Decodable;
use elements::hex::FromHex;
use elements::pset::PartiallySignedTransaction;
use elements::{AssetId, Txid};
use elements::{Block, TxOutSecrets};
use elements_miniscript::descriptor::checksum::desc_checksum;
use std::{
    io::{Read, Write},
    net::TcpListener,
    str::FromStr,
    thread,
    time::Duration,
};

mod amp2;
mod auth;
mod fee;
mod generate;
mod lightningd;
mod panic_store;
mod pegin;
mod registry;
mod test_env;
mod waterfalls;
pub use auth::{
    AuthStack, AUTH_CLIENT_ID, AUTH_CLIENT_SECRET, AUTH_REALM, AUTH_SHORT_CLIENT_ID,
    AUTH_SHORT_CLIENT_SECRET, AUTH_USER_UUID,
};
pub use fee::{assert_fee_rate, compute_fee_rate, compute_fee_rate_without_discount_ct};
pub use generate::{generate_mnemonic, generate_slip77, generate_view_key, generate_xprv};
pub use panic_store::PanicStore;
pub use pegin::{
    FED_PEG_DESC, FED_PEG_SCRIPT, FED_PEG_SCRIPT_ASM, PEGIN_TEST_ADDR, PEGIN_TEST_DESC,
};
pub use test_env::{TestEnv, TestEnvBuilder};

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
pub const TEST_MNEMONIC_XPUB: &str =
"tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s";
pub const TEST_MNEMONIC_SLIP77: &str =
    "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

pub const DEFAULT_SPECULOS_MNEMONIC: &str = "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin";

/// Descriptor with 11 txs on testnet
pub const TEST_DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

/// Start a tiny local HTTP server for tests and return its base URL.
///
/// The server responds with the same status, content type, and body for every
/// request. If `keep_open` is true it keeps each accepted connection open after
/// writing the body, which is useful for testing stream cancellation.
pub fn serve_http_response(
    status_line: &'static str,
    content_type: &'static str,
    body: &'static str,
    keep_open: bool,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || loop {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = stream.write_all(response.as_bytes());
        if keep_open {
            thread::sleep(Duration::from_secs(30));
        }
    });
    format!("http://{addr}")
}

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

pub fn add_checksum(desc: &str) -> String {
    if desc.find('#').is_some() {
        desc.into()
    } else {
        format!("{}#{}", desc, desc_checksum(desc).unwrap())
    }
}

/// Serialize and deserialize a PSET
///
/// This allows us to catch early (de)serialization issues,
/// which can be hit in practice since PSETs are passed around as b64 strings.
pub fn pset_rt(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    PartiallySignedTransaction::from_str(&pset.to_string()).unwrap()
}

pub fn regtest_policy_asset() -> AssetId {
    AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225").unwrap()
}

pub fn init_logging() {
    let _ = env_logger::try_init();
}

pub fn n_issuances(details: &lwk_common::PsetDetails) -> usize {
    details
        .issuances()
        .iter()
        .filter(|e| e.is_issuance())
        .count()
}

pub fn n_reissuances(details: &lwk_common::PsetDetails) -> usize {
    details
        .issuances()
        .iter()
        .filter(|e| e.is_reissuance())
        .count()
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

pub fn wollet_descriptor_string2() -> String {
    include_str!("../test_data/update_test_vector/desc").to_string()
}

pub fn wollet_descriptor_string() -> String {
    include_str!("../test_data/update_test_vector/desc2").to_string()
}

pub fn wollet_descriptor_many_transactions() -> &'static str {
    "ct(slip77(ac53739ddde9fdf6bba3dbc51e989b09aa8c9cdce7b7d7eddd49cec86ddf71f7),elwpkh([93970d14/84'/1'/0']tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))#u0y4axgs"
}

/// A 3 of 5 descriptor and a vector of partially signed transactions to combine 1 sig each
pub fn psets_to_combine() -> (String, Vec<PartiallySignedTransaction>) {
    let c = |s: &str| PartiallySignedTransaction::from_str(s).unwrap();
    let ps = vec![
        c(include_str!("../test_data/pset_combine/s1_pset.base64")),
        c(include_str!("../test_data/pset_combine/s2_pset.base64")),
        c(include_str!("../test_data/pset_combine/s3_pset.base64")),
        c(include_str!("../test_data/pset_combine/s4_pset.base64")),
        c(include_str!("../test_data/pset_combine/s5_pset.base64")),
    ];
    let d = include_str!("../test_data/pset_combine/desc");
    (d.to_string(), ps)
}

pub fn descriptor_pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/desc")
}

/// Pset created with descriptor [`descriptor_pset_usdt_no_contracts`] containing mainnet USDt but without contract info
pub fn pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_no_contracts.base64")
}

pub fn pset_usdt_with_contract() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_with_contract.base64")
}
