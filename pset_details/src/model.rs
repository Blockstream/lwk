use elements_miniscript::elements::bitcoin::{bip32::KeySource, key::PublicKey};
use elements_miniscript::elements::AssetId;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PsetBalance {
    pub fee: u64,
    pub balances: HashMap<AssetId, i64>,
}

#[derive(Debug)]
pub struct PsetSignatures {
    pub has_signature: Vec<(PublicKey, KeySource)>,
    pub missing_signature: Vec<(PublicKey, KeySource)>,
}

#[derive(Debug)]
pub struct PsetDetails {
    pub balance: PsetBalance,

    /// For each input, existing or missing signatures
    pub sig_details: Vec<PsetSignatures>,
}
