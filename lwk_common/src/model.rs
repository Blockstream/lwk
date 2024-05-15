use elements_miniscript::elements::bitcoin::{
    bip32::{Fingerprint, KeySource},
    key::PublicKey,
};
use elements_miniscript::elements::pset::Input;
use elements_miniscript::elements::secp256k1_zkp::ZERO_TWEAK;
use elements_miniscript::elements::{AssetId, AssetIssuance, OutPoint, Txid};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct PsetBalance {
    pub fee: u64,
    pub balances: BTreeMap<AssetId, i64>,
}

#[derive(Debug, Clone)]
pub struct PsetSignatures {
    pub has_signature: Vec<(PublicKey, KeySource)>,
    pub missing_signature: Vec<(PublicKey, KeySource)>,
}

#[derive(Debug, Clone)]
pub struct Issuance {
    asset: AssetId,
    token: AssetId,
    prev_output: OutPoint,
    inner: AssetIssuance,
}

impl Issuance {
    pub fn new(input: &Input) -> Self {
        // There are meaningless if inner is null
        let (asset, token) = input.issuance_ids();
        let prev_output = OutPoint::new(input.previous_txid, input.previous_output_index);
        Self {
            asset,
            token,
            prev_output,
            inner: input.asset_issuance(),
        }
    }

    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    pub fn is_issuance(&self) -> bool {
        !self.is_null() && self.inner.asset_blinding_nonce == ZERO_TWEAK
    }

    pub fn is_reissuance(&self) -> bool {
        !self.is_null() && self.inner.asset_blinding_nonce != ZERO_TWEAK
    }

    pub fn is_confidential(&self) -> bool {
        self.inner.amount.is_confidential() || self.inner.inflation_keys.is_confidential()
    }

    pub fn asset_satoshi(&self) -> Option<u64> {
        self.inner.amount.explicit()
    }

    pub fn token_satoshi(&self) -> Option<u64> {
        self.inner.inflation_keys.explicit()
    }

    pub fn asset(&self) -> Option<AssetId> {
        (!self.is_null()).then_some(self.asset)
    }

    pub fn token(&self) -> Option<AssetId> {
        (!self.is_null()).then_some(self.token)
    }

    pub fn prev_txid(&self) -> Option<Txid> {
        (!self.is_null()).then_some(self.prev_output.txid)
    }

    pub fn prev_vout(&self) -> Option<u32> {
        (!self.is_null()).then_some(self.prev_output.vout)
    }
}

#[derive(Debug, Clone)]
pub struct PsetDetails {
    pub balance: PsetBalance,

    /// For each input, existing or missing signatures
    pub sig_details: Vec<PsetSignatures>,

    /// For each input, the corresponding issuance
    pub issuances: Vec<Issuance>,
}

impl PsetDetails {
    /// Set of fingerprints for which the PSET has a signature
    pub fn fingerprints_has(&self) -> BTreeSet<Fingerprint> {
        let mut r = BTreeSet::new();
        for sigs in &self.sig_details {
            for (_, (fingerprint, _)) in &sigs.has_signature {
                r.insert(*fingerprint);
            }
        }
        r
    }

    /// Set of fingerprints for which the PSET is missing a signature
    pub fn fingerprints_missing(&self) -> BTreeSet<Fingerprint> {
        let mut r = BTreeSet::new();
        for sigs in &self.sig_details {
            for (_, (fingerprint, _)) in &sigs.missing_signature {
                r.insert(*fingerprint);
            }
        }
        r
    }
}
