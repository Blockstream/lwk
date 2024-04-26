use std::collections::HashMap;

use lwk_wollet::{bitcoin::bip32::KeySource, elements};
use wasm_bindgen::prelude::*;

use crate::{AssetId, Txid};

/// PSET details from a perspective of a wallet, wrapper of [`lwk_common::PsetDetails`]
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PsetDetails {
    inner: lwk_common::PsetDetails,
}

/// PSET details from a perspective of a wallet, wrapper of [`lwk_common::PsetBalance`]
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PsetBalance {
    inner: lwk_common::PsetBalance,
}

/// PSET details from a perspective of a wallet, wrapper of [`lwk_common::PsetSignatures`]
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PsetSignatures {
    inner: lwk_common::PsetSignatures,
}

/// PSET details from a perspective of a wallet, wrapper of [`lwk_common::Issuance`]
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Issuance {
    inner: lwk_common::Issuance,
}

#[wasm_bindgen]
impl PsetDetails {
    pub fn balance(&self) -> PsetBalance {
        self.inner.balance.clone().into()
    }
    pub fn signatures(&self) -> Vec<PsetSignatures> {
        self.inner
            .sig_details
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Return an element for every input that could possibly be a issuance or a reissuance
    pub fn inputs_issuances(&self) -> Vec<Issuance> {
        // this is not aligned with what we are doing in app, where we offer a vec of only issuance and another with only reissuance
        // with a reference to the relative input. We should problaby move that logic upper so we can reuse?
        // in the meantime, this less ergonomic method should suffice.
        self.inner
            .issuances
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }
}

#[wasm_bindgen]
impl PsetBalance {
    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    /// The net balance for every asset with respect of the wallet asking the pset details
    pub fn balances(&self) -> JsValue {
        serde_wasm_bindgen::to_value(
            &self
                .inner
                .balances
                .iter()
                .collect::<HashMap<&elements::AssetId, &i64>>(),
        )
        .expect("should map")
    }
}

#[wasm_bindgen]
impl PsetSignatures {
    ///Vec<(PublicKey, KeySource)>
    pub fn has_signature(&self) -> JsValue {
        convert(&self.inner.has_signature)
    }
    pub fn missing_signature(&self) -> JsValue {
        convert(&self.inner.missing_signature)
    }
}
fn convert(data: &[(elements::bitcoin::PublicKey, KeySource)]) -> JsValue {
    serde_wasm_bindgen::to_value(
        &data
            .iter()
            .map(|(a, b)| (a.to_string(), b.0.to_string())) // TODO include derivation path
            .collect::<Vec<_>>(),
    )
    .expect("should map")
}

#[wasm_bindgen]
impl Issuance {
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset().map(Into::into)
    }
    pub fn token(&self) -> Option<AssetId> {
        self.inner.token().map(Into::into)
    }
    pub fn prev_vout(&self) -> Option<u32> {
        self.inner.prev_vout().map(Into::into)
    }
    pub fn prev_txid(&self) -> Option<Txid> {
        self.inner.prev_txid().map(Into::into)
    }
    pub fn is_issuance(&self) -> bool {
        self.inner.is_issuance()
    }
    pub fn is_reissuance(&self) -> bool {
        self.inner.is_reissuance()
    }
}

impl From<PsetDetails> for lwk_common::PsetDetails {
    fn from(pset_details: PsetDetails) -> Self {
        pset_details.inner
    }
}

impl From<lwk_common::PsetDetails> for PsetDetails {
    fn from(pset_details: lwk_common::PsetDetails) -> Self {
        Self {
            inner: pset_details,
        }
    }
}

impl From<PsetBalance> for lwk_common::PsetBalance {
    fn from(pset_balance: PsetBalance) -> Self {
        pset_balance.inner
    }
}

impl From<lwk_common::PsetBalance> for PsetBalance {
    fn from(pset_balance: lwk_common::PsetBalance) -> Self {
        Self {
            inner: pset_balance,
        }
    }
}

impl From<PsetSignatures> for lwk_common::PsetSignatures {
    fn from(pset_sigs: PsetSignatures) -> Self {
        pset_sigs.inner
    }
}

impl From<lwk_common::PsetSignatures> for PsetSignatures {
    fn from(pset_sigs: lwk_common::PsetSignatures) -> Self {
        Self { inner: pset_sigs }
    }
}

impl From<Issuance> for lwk_common::Issuance {
    fn from(pset_iss: Issuance) -> Self {
        pset_iss.inner
    }
}

impl From<lwk_common::Issuance> for Issuance {
    fn from(pset_iss: lwk_common::Issuance) -> Self {
        Self { inner: pset_iss }
    }
}
