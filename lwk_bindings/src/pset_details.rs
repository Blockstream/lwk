use std::{collections::HashMap, sync::Arc};

use crate::{types::AssetId, Address, Txid};

/// The details of a Partially Signed Elements Transaction:
///
/// - the net balance from the point of view of the wallet
/// - the available and missing signatures for each input
/// - for issuances and reissuances transactions contains the issuance or reissuance details
#[derive(uniffi::Object, Debug)]
pub struct PsetDetails {
    inner: lwk_common::PsetDetails,
}

impl From<lwk_common::PsetDetails> for PsetDetails {
    fn from(inner: lwk_common::PsetDetails) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl PsetDetails {
    /// Return the balance of the PSET from the point of view of the wallet
    /// that generated this via `psetDetails()`
    pub fn balance(&self) -> Arc<PsetBalance> {
        Arc::new(self.inner.balance.clone().into())
    }

    /// For each input its existing or missing signatures
    pub fn signatures(&self) -> Vec<Arc<PsetSignatures>> {
        self.inner
            .sig_details
            .clone()
            .into_iter()
            .map(|s| Arc::new(s.into()))
            .collect()
    }

    /// Return an element for every input that could possibly be a issuance or a reissuance
    pub fn inputs_issuances(&self) -> Vec<Arc<Issuance>> {
        // this is not aligned with what we are doing in app, where we offer a vec of only issuance and another with only reissuance
        // with a reference to the relative input. We should problaby move that logic upper so we can reuse?
        // in the meantime, this less ergonomic method should suffice.
        self.inner
            .issuances
            .clone()
            .into_iter()
            .map(|e| Arc::new(e.into()))
            .collect()
    }

    /// Set of fingerprints for which the PSET has a signature
    pub fn fingerprints_has(&self) -> Vec<String> {
        // TODO: return HashSet when upgrading to uniffi-rs 0.29 or later
        self.inner
            .fingerprints_has()
            .into_iter()
            .map(|fp| fp.to_string())
            .collect()
    }

    /// Set of fingerprints for which the PSET is missing a signature
    pub fn fingerprints_missing(&self) -> Vec<String> {
        // TODO: return HashSet when upgrading to uniffi-rs 0.29 or later
        self.inner
            .fingerprints_missing()
            .into_iter()
            .map(|fp| fp.to_string())
            .collect()
    }
}

#[derive(uniffi::Object, Debug)]
pub struct PsetBalance {
    inner: lwk_common::PsetBalance,
}

impl From<lwk_common::PsetBalance> for PsetBalance {
    fn from(inner: lwk_common::PsetBalance) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl PsetBalance {
    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    pub fn balances(&self) -> HashMap<AssetId, i64> {
        self.inner
            .balances
            .iter()
            .map(|(k, v)| ((*k).into(), *v))
            .collect()
    }

    pub fn recipients(&self) -> Vec<Arc<Recipient>> {
        self.inner
            .recipients
            .clone()
            .into_iter()
            .map(|e| Arc::new(e.into()))
            .collect()
    }
}

#[derive(uniffi::Object, Debug)]
pub struct PsetSignatures {
    inner: lwk_common::PsetSignatures,
}

impl From<lwk_common::PsetSignatures> for PsetSignatures {
    fn from(inner: lwk_common::PsetSignatures) -> Self {
        Self { inner }
    }
}

type PublicKey = String;
type KeySource = String;

#[uniffi::export]
impl PsetSignatures {
    pub fn has_signature(&self) -> HashMap<PublicKey, KeySource> {
        self.inner
            .has_signature
            .iter()
            .map(|(k, v)| (k.to_string(), key_source_to_string(v)))
            .collect()
    }

    pub fn missing_signature(&self) -> HashMap<PublicKey, KeySource> {
        self.inner
            .missing_signature
            .iter()
            .map(|(k, v)| (k.to_string(), key_source_to_string(v)))
            .collect()
    }
}

fn key_source_to_string(
    key_source: &(
        elements::bitcoin::bip32::Fingerprint,
        elements::bitcoin::bip32::DerivationPath,
    ),
) -> String {
    format!("[{}]{}", key_source.0, key_source.1)
}

/// The details of an issuance or reissuance
#[derive(uniffi::Object, Debug)]

pub struct Issuance {
    inner: lwk_common::Issuance,
}

#[uniffi::export]
impl Issuance {
    /// Return the asset id or None if it's a null issuance
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset().map(Into::into)
    }

    /// Return the token id or None if it's a null issuance
    pub fn token(&self) -> Option<AssetId> {
        self.inner.token().map(Into::into)
    }

    /// Return the previous output index or None if it's a null issuance
    pub fn prev_vout(&self) -> Option<u32> {
        self.inner.prev_vout()
    }

    /// Return the previous transaction id or None if it's a null issuance
    pub fn prev_txid(&self) -> Option<Arc<Txid>> {
        self.inner.prev_txid().map(|e| Arc::new(e.into()))
    }

    /// Return true if the issuance or reissuance is null
    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    /// Return true if this is effectively an issuance
    pub fn is_issuance(&self) -> bool {
        self.inner.is_issuance()
    }

    /// Return true if this is effectively a reissuance
    pub fn is_reissuance(&self) -> bool {
        self.inner.is_reissuance()
    }

    /// Return true if the issuance or reissuance is confidential
    pub fn is_confidential(&self) -> bool {
        self.inner.is_confidential()
    }

    /// Return the amount of the asset in satoshis
    pub fn asset_satoshi(&self) -> Option<u64> {
        self.inner.asset_satoshi()
    }

    /// Return the amount of the reissuance token in satoshis
    pub fn token_satoshi(&self) -> Option<u64> {
        self.inner.token_satoshi()
    }
}

impl From<lwk_common::Issuance> for Issuance {
    fn from(inner: lwk_common::Issuance) -> Self {
        Self { inner }
    }
}

#[derive(uniffi::Object, Debug)]
pub struct Recipient {
    inner: lwk_common::Recipient,
}

impl From<lwk_common::Recipient> for Recipient {
    fn from(inner: lwk_common::Recipient) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl Recipient {
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset.map(Into::into)
    }

    pub fn value(&self) -> Option<u64> {
        self.inner.value
    }

    pub fn address(&self) -> Option<Arc<Address>> {
        self.inner
            .address
            .as_ref()
            .map(|e| Arc::new(e.clone().into()))
    }
    pub fn vout(&self) -> u32 {
        self.inner.vout
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{types::AssetId, Network, Pset, Wollet, WolletDescriptor};

    #[test]
    fn pset_details() {
        let pset = include_str!("../test_data/pset_details/pset.base64");
        let pset = Pset::new(pset).unwrap();

        let descriptor = include_str!("../test_data/pset_details/desc");
        let descriptor = WolletDescriptor::new(descriptor).unwrap();
        let network = Network::regtest_default();
        let wollet = Wollet::new(&network, &descriptor, None).unwrap();

        let details = wollet.pset_details(&pset).unwrap();
        assert_eq!(details.balance().fee(), 254);

        let balances = details.balance().balances();
        assert_eq!(balances.len(), 1);
        let expected_asset_id = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset_id = elements::AssetId::from_str(expected_asset_id).unwrap();
        let asset_id: AssetId = asset_id.into();
        let val = balances.get(&asset_id).unwrap();
        assert_eq!(*val, -1254);

        let signatures = details.signatures();
        assert_eq!(signatures.len(), 1);

        assert_eq!(format!("{:?}", signatures[0].has_signature()), "{\"02ab89406d9cf32ff1819838136eecb65c07add8e8ef1cd2d6c64bab1d85606453\": \"[6e055509]87'/1'/0'/0/0\"}");
        assert_eq!(format!("{:?}", signatures[0].missing_signature()), "{\"03c1d0c7ddab5bd5bffbe0bf04a8a570eeabd9b6356358ecaacc242f658c7d5aad\": \"[281e2239]87'/1'/0'/0/0\"}");

        let issuances = details.inputs_issuances();
        assert_eq!(issuances.len(), 1);
        assert!(!issuances[0].is_issuance());
        assert!(!issuances[0].is_reissuance());
        assert!(issuances[0].is_null());
        assert!(!issuances[0].is_confidential());

        let recipients = details.balance().recipients();
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].vout(), 0);
        assert_eq!(
            recipients[0].asset().unwrap().to_string(),
            "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225"
        );
        assert_eq!(recipients[0].value(), Some(1000));
        assert_eq!(
            recipients[0].address().unwrap().to_string(),
            "AzpoyU5wJFcfdq6sh5ETbqCBA1oLuoLYk5UGJbYLGj3wKMurrVQiX1Djq67JHFAVt1hA5QVq41iNuVmy"
        );

        assert_eq!(details.fingerprints_has(), vec!["6e055509"]);
        assert_eq!(details.fingerprints_missing(), vec!["281e2239"]);
    }
}
