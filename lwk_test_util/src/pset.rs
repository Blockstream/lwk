use elements_miniscript::elements::{self};

use elements::pset::PartiallySignedTransaction;
use std::str::FromStr;

/// Serialize and deserialize a PSET
///
/// This allows us to catch early (de)serialization issues,
/// which can be hit in practice since PSETs are passed around as b64 strings.
pub fn pset_rt(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    PartiallySignedTransaction::from_str(&pset.to_string()).unwrap()
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
