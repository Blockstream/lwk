use elements_miniscript::elements::bitcoin::{
    bip32::{Fingerprint, KeySource},
    key::PublicKey,
};
use elements_miniscript::elements::pset::{Input, PartiallySignedTransaction};
use elements_miniscript::elements::secp256k1_zkp::ZERO_TWEAK;
use elements_miniscript::elements::{AssetId, AssetIssuance, OutPoint, Txid};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use std::collections::{BTreeSet, HashMap};

use crate::{
    pset_balance, pset_has_non_default_sighash, pset_issuances, pset_signatures, Error, Network,
    SignedBalance,
};

/// The details regarding balance and amounts in a PSET
#[derive(Debug, Clone)]
pub struct PsetBalance {
    fees: HashMap<AssetId, u64>,
    balances: SignedBalance,
    recipients: Vec<Recipient>,
}

impl PsetBalance {
    /// Create the balance details of a PSET
    pub fn new(
        fees: HashMap<AssetId, u64>,
        balances: SignedBalance,
        recipients: Vec<Recipient>,
    ) -> Self {
        Self {
            fees,
            balances,
            recipients,
        }
    }

    /// The fees of the transaction in the PSET
    pub fn fees(&self) -> &HashMap<AssetId, u64> {
        &self.fees
    }

    /// The net balance of the assets in the PSET from the point of view of the wallet
    pub fn balances(&self) -> &SignedBalance {
        &self.balances
    }

    /// Outputs going out of the wallet
    pub fn recipients(&self) -> &[Recipient] {
        &self.recipients
    }

    /// Return the amount of fee with given asset id
    pub fn fees_in(&self, asset: &AssetId) -> u64 {
        *self.fees.get(asset).unwrap_or(&0)
    }
}

/// The recipient (an output not belonging to the wallet) in a PSET
#[derive(Debug, Clone)]
pub struct Recipient {
    address: Option<elements::Address>,
    asset: Option<AssetId>,
    value: Option<u64>,
    vout: u32,
}

impl Recipient {
    /// Create a recipient of a PSET
    pub fn new(
        address: Option<elements::Address>,
        asset: Option<AssetId>,
        value: Option<u64>,
        vout: u32,
    ) -> Self {
        Self {
            address,
            asset,
            value,
            vout,
        }
    }

    /// The confidential address of the recipients.
    ///
    /// Can be None in the following cases:
    ///  - if no blinding key is available in the PSET, FIXME?
    ///  - if the script is not a known template
    pub fn address(&self) -> Option<&elements::Address> {
        self.address.as_ref()
    }

    /// The asset sent to this recipient if it's available to extract from the PSET
    pub fn asset(&self) -> Option<AssetId> {
        self.asset
    }

    /// The value sent to this recipient if it's available to extract from the PSET
    pub fn value(&self) -> Option<u64> {
        self.value
    }

    /// The index of the output in the transaction
    pub fn vout(&self) -> u32 {
        self.vout
    }
}

/// The details of the signatures in a PSET
#[derive(Debug, Clone)]
pub struct PsetSignatures {
    has_signature: Vec<(PublicKey, KeySource)>,
    missing_signature: Vec<(PublicKey, KeySource)>,
}

impl PsetSignatures {
    /// Create the signature details of a PSET input
    pub fn new(
        has_signature: Vec<(PublicKey, KeySource)>,
        missing_signature: Vec<(PublicKey, KeySource)>,
    ) -> Self {
        Self {
            has_signature,
            missing_signature,
        }
    }

    /// The signatures that are available
    pub fn has_signature(&self) -> &[(PublicKey, KeySource)] {
        &self.has_signature
    }

    /// The signatures that are missing
    pub fn missing_signature(&self) -> &[(PublicKey, KeySource)] {
        &self.missing_signature
    }
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
    balance: PsetBalance,
    sig_details: Vec<PsetSignatures>,
    issuances: Vec<Issuance>,
    has_non_default_sighash: bool,
}

impl PsetDetails {
    /// Compute the details of the given PSET with respect to the given descriptor
    pub fn new(
        pset: &PartiallySignedTransaction,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
        network: &Network,
    ) -> Result<Self, Error> {
        Ok(Self {
            balance: pset_balance(pset, descriptor, network.address_params())?,
            sig_details: pset_signatures(pset),
            issuances: pset_issuances(pset),
            has_non_default_sighash: pset_has_non_default_sighash(pset),
        })
    }

    /// The net balance of the PSET from the point of view of the wallet
    pub fn balance(&self) -> &PsetBalance {
        &self.balance
    }

    /// The fees of the transaction in the PSET
    pub fn fees(&self) -> &HashMap<AssetId, u64> {
        self.balance.fees()
    }

    /// Return the amount of fee with given asset id
    pub fn fees_in(&self, asset: &AssetId) -> u64 {
        self.balance.fees_in(asset)
    }

    /// The net balance of the assets in the PSET from the point of view of the wallet
    pub fn balances(&self) -> &SignedBalance {
        self.balance.balances()
    }

    /// Outputs going out of the wallet
    pub fn recipients(&self) -> &[Recipient] {
        self.balance.recipients()
    }

    /// For each input, existing or missing signatures
    pub fn sig_details(&self) -> &[PsetSignatures] {
        &self.sig_details
    }

    /// For each input, the corresponding issuance
    pub fn issuances(&self) -> &[Issuance] {
        &self.issuances
    }

    /// Whether any PSET input sighash is not the default one
    pub fn has_non_default_sighash(&self) -> bool {
        self.has_non_default_sighash
    }

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
