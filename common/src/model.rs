use elements_miniscript::elements::bitcoin::{
    bip32::{Fingerprint, KeySource},
    key::PublicKey,
};
use elements_miniscript::elements::secp256k1_zkp::ZERO_TWEAK;
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

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn is_issuance(&self) -> bool {
        !self.is_null() && self.0.asset_blinding_nonce == ZERO_TWEAK
    }

    pub fn is_reissuance(&self) -> bool {
        !self.is_null() && self.0.asset_blinding_nonce != ZERO_TWEAK
    }

    pub fn is_blinded(&self) -> bool {
        self.0.amount.is_confidential() || self.0.inflation_keys.is_confidential()
    }

    pub fn asset_satoshi(&self) -> Option<u64> {
        self.0.amount.explicit()
    }

    pub fn token_satoshi(&self) -> Option<u64> {
        self.0.inflation_keys.explicit()
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
