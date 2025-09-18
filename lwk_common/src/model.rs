use elements_miniscript::elements::bitcoin::{
    bip32::{Fingerprint, KeySource},
    key::PublicKey,
};
use elements_miniscript::elements::pset::Input;
use elements_miniscript::elements::secp256k1_zkp::ZERO_TWEAK;
use elements_miniscript::elements::{AssetId, AssetIssuance, OutPoint, Txid};
use std::collections::BTreeSet;

use crate::SignedBalance;

/// The details regarding balance and amounts in a PSET
#[derive(Debug, Clone)]
pub struct PsetBalance {
    /// The fee of the transaction in the PSET
    pub fee: u64,

    /// The net balance of the assets in the PSET from the point of view of the wallet
    pub balances: SignedBalance,

    /// Outputs going out of the wallet
    pub recipients: Vec<Recipient>,
}

/// The recipient (an output not belonging to the wallet) in a PSET
#[derive(Debug, Clone)]
pub struct Recipient {
    /// The confidential address of the recipients.
    ///
    /// Can be None in the following cases:
    ///  - if no blinding key is available in the PSET, FIXME?
    ///  - if the script is not a known template
    pub address: Option<elements::Address>,

    /// The asset sent to this recipient if it's available to extract from the PSET
    pub asset: Option<AssetId>,

    /// The value sent to this recipient if it's available to extract from the PSET
    pub value: Option<u64>,

    /// The index of the output in the transaction
    pub vout: u32,
}

/// The details of the signatures in a PSET
#[derive(Debug, Clone)]
pub struct PsetSignatures {
    /// The signatures that are available
    pub has_signature: Vec<(PublicKey, KeySource)>,

    /// The signatures that are missing
    pub missing_signature: Vec<(PublicKey, KeySource)>,
}

/// The details of an issuance or reissuance
#[derive(Debug, Clone)]
pub struct Issuance {
    asset: AssetId,
    token: AssetId,
    prev_output: OutPoint,
    inner: AssetIssuance,
}

impl Issuance {
    /// Create a new issuance or reissuance information from a input of a PSET
    pub fn new(input: &Input) -> Self {
        // TODO: return Result<Self, Error> and error if input.asset_issuance() is null, adjust documentation/function also in pset_issuances()

        // These are meaningless if inner is null
        let (asset, token) = input.issuance_ids();
        let prev_output = OutPoint::new(input.previous_txid, input.previous_output_index);
        Self {
            asset,
            token,
            prev_output,
            inner: input.asset_issuance(),
        }
    }

    /// Return true if the issuance or reissuance is null
    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    /// Return true if this is effectively an issuance
    pub fn is_issuance(&self) -> bool {
        !self.is_null() && self.inner.asset_blinding_nonce == ZERO_TWEAK
    }

    /// Return true if this is effectively a reissuance
    pub fn is_reissuance(&self) -> bool {
        !self.is_null() && self.inner.asset_blinding_nonce != ZERO_TWEAK
    }

    /// Return true if the issuance or reissuance is confidential
    pub fn is_confidential(&self) -> bool {
        self.inner.amount.is_confidential() || self.inner.inflation_keys.is_confidential()
    }

    /// Return the amount of the asset in satoshis
    pub fn asset_satoshi(&self) -> Option<u64> {
        self.inner.amount.explicit()
    }

    /// Return the amount of the reissuance token in satoshis
    pub fn token_satoshi(&self) -> Option<u64> {
        self.inner.inflation_keys.explicit()
    }

    /// Return the asset id or None if it's a null issuance
    pub fn asset(&self) -> Option<AssetId> {
        (!self.is_null()).then_some(self.asset)
    }

    /// Return the token id or None if it's a null issuance
    pub fn token(&self) -> Option<AssetId> {
        (!self.is_null()).then_some(self.token)
    }

    /// Return the previous transaction id or None if it's a null issuance
    pub fn prev_txid(&self) -> Option<Txid> {
        (!self.is_null()).then_some(self.prev_output.txid)
    }

    /// Return the previous output index or None if it's a null issuance
    pub fn prev_vout(&self) -> Option<u32> {
        (!self.is_null()).then_some(self.prev_output.vout)
    }
}

/// The details of a Partially Signed Elements Transaction:
///
/// - the net balance from the point of view of the wallet
/// - the available and missing signatures for each input
/// - for issuances and reissuances transactions contains the issuance or reissuance details
#[derive(Debug, Clone)]
pub struct PsetDetails {
    /// The net balance of the PSET from the point of view of the wallet
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
