use std::collections::HashMap;

use elements_miniscript::{
    elements::{pset::PartiallySignedTransaction, AssetId},
    ConfidentialDescriptor, DescriptorPublicKey,
};

pub struct PsetBalance {
    _fee: u64,
    _balances: HashMap<AssetId, i64>,
}

pub enum Error {}

pub fn pset_balance(
    _pset: &PartiallySignedTransaction,
    _desc: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<PsetBalance, Error> {
    todo!()
}
