use lwk_wollet::{bitcoin::bip32::KeySource, elements};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

use crate::{AssetId, Error, Txid};

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

    /// For each input existing or missing signatures
    pub fn signatures(&self) -> Vec<PsetSignatures> {
        self.inner
            .sig_details
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[wasm_bindgen(js_name = fingerprintsMissing)]
    pub fn fingerprints_missing(&self) -> Vec<String> {
        self.inner
            .fingerprints_missing()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[wasm_bindgen(js_name = fingerprintsHas)]
    pub fn fingerprints_has(&self) -> Vec<String> {
        self.inner
            .fingerprints_has()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Return an element for every input that could possibly be a issuance or a reissuance
    #[wasm_bindgen(js_name = inputsIssuances)]
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
    pub fn balances(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);
        Ok(self.inner.balances.serialize(&serializer)?)
    }
}

#[wasm_bindgen]
impl PsetSignatures {
    /// Returns `Vec<(PublicKey, KeySource)>`
    #[wasm_bindgen(js_name = hasSignature)]
    pub fn has_signature(&self) -> JsValue {
        convert(&self.inner.has_signature)
    }

    #[wasm_bindgen(js_name = missingSignature)]
    pub fn missing_signature(&self) -> JsValue {
        convert(&self.inner.missing_signature)
    }
}
fn convert(data: &[(elements::bitcoin::PublicKey, KeySource)]) -> JsValue {
    serde_wasm_bindgen::to_value(
        &data
            .iter()
            .map(|(a, b)| (a.to_string(), b.0.to_string(), b.1.to_string())) // TODO include derivation path
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

    #[wasm_bindgen(js_name = prevVout)]
    pub fn prev_vout(&self) -> Option<u32> {
        self.inner.prev_vout().map(Into::into)
    }

    #[wasm_bindgen(js_name = prevTxid)]
    pub fn prev_txid(&self) -> Option<Txid> {
        self.inner.prev_txid().map(Into::into)
    }

    #[wasm_bindgen(js_name = isIssuance)]
    pub fn is_issuance(&self) -> bool {
        self.inner.is_issuance()
    }

    #[wasm_bindgen(js_name = isReissuance)]
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

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use std::collections::HashMap;

    use wasm_bindgen_test::*;

    use crate::{Network, Pset, Wollet, WolletDescriptor};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_pset_details() {
        let pset = include_str!("../test_data/pset_details/pset.base64");
        let pset = Pset::new(pset).unwrap();

        let descriptor = include_str!("../test_data/pset_details/desc");
        let descriptor = WolletDescriptor::new(descriptor).unwrap();
        let network = Network::regtest_default();
        let wollet = Wollet::new(&network, &descriptor).unwrap();

        let details = wollet.pset_details(&pset).unwrap();
        assert_eq!(details.balance().fee(), 254);
        let balance: HashMap<lwk_wollet::elements::AssetId, i64> =
            serde_wasm_bindgen::from_value(details.balance().balances().unwrap()).unwrap();
        assert_eq!(
            format!("{:?}", balance),
            "{5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225: -1254}"
        );

        let signatures = details.signatures();
        assert_eq!(signatures.len(), 1);

        assert_eq!(format!("{:?}", signatures[0].has_signature()), "JsValue([[\"02ab89406d9cf32ff1819838136eecb65c07add8e8ef1cd2d6c64bab1d85606453\", \"6e055509\", \"87'/1'/0'/0/0\"]])");
        assert_eq!(format!("{:?}", signatures[0].missing_signature()), "JsValue([[\"03c1d0c7ddab5bd5bffbe0bf04a8a570eeabd9b6356358ecaacc242f658c7d5aad\", \"281e2239\", \"87'/1'/0'/0/0\"]])");

        let issuances = details.inputs_issuances();
        assert_eq!(issuances.len(), 1);
        assert!(!issuances[0].is_issuance());
        assert!(!issuances[0].is_reissuance());
    }
}
