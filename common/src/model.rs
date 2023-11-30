use elements_miniscript::elements::bitcoin::{
    bip32::{Fingerprint, KeySource},
    key::PublicKey,
};
use elements_miniscript::elements::{AssetId, AssetIssuance};
use std::collections::{HashMap, HashSet};

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

/// Wrapper around `AssetIssuance` to extract data more nicely
#[derive(Debug)]
pub struct Issuance(AssetIssuance);

impl Issuance {
    pub fn new(issuance: AssetIssuance) -> Self {
        Self(issuance)
    }
}

#[derive(Debug)]
pub struct PsetDetails {
    pub balance: PsetBalance,

    /// For each input, existing or missing signatures
    pub sig_details: Vec<PsetSignatures>,

    /// For each input, the corresponding issuance
    pub issuances: Vec<Issuance>,
}

impl PsetDetails {
    /// Set of fingerprints for which the PSET has a signature
    pub fn fingerprints_has(&self) -> HashSet<Fingerprint> {
        let mut r = HashSet::new();
        for sigs in &self.sig_details {
            for (_, (fingerprint, _)) in &sigs.has_signature {
                r.insert(*fingerprint);
            }
        }
        r
    }

    /// Set of fingerprints for which the PSET is missing a signature
    pub fn fingerprints_missing(&self) -> HashSet<Fingerprint> {
        let mut r = HashSet::new();
        for sigs in &self.sig_details {
            for (_, (fingerprint, _)) in &sigs.missing_signature {
                r.insert(*fingerprint);
            }
        }
        r
    }
}
