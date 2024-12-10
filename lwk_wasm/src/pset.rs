use crate::{AssetId, Error, Transaction, Txid};
use lwk_wollet::elements::pset::{Input, PartiallySignedTransaction};
use std::fmt::Display;
use wasm_bindgen::prelude::*;

/// Partially Signed Elements Transaction, wrapper of [`PartiallySignedTransaction`]
#[wasm_bindgen]
#[derive(PartialEq, Debug, Clone)]
pub struct Pset {
    inner: PartiallySignedTransaction,
}

impl From<PartiallySignedTransaction> for Pset {
    fn from(inner: PartiallySignedTransaction) -> Self {
        Self { inner }
    }
}

impl From<Pset> for PartiallySignedTransaction {
    fn from(pset: Pset) -> Self {
        pset.inner
    }
}

impl Display for Pset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Pset {
    /// Creates a `Pset`
    #[wasm_bindgen(constructor)]
    pub fn new(base64: &str) -> Result<Pset, Error> {
        let pset: PartiallySignedTransaction = base64.trim().parse()?;
        Ok(pset.into())
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }

    #[wasm_bindgen(js_name = extractTx)]
    pub fn extract_tx(&self) -> Result<Transaction, Error> {
        let tx: Transaction = self.inner.extract_tx()?.into();
        Ok(tx)
    }

    pub fn combine(&mut self, other: Pset) -> Result<(), Error> {
        self.inner.merge(other.into())?;
        Ok(())
    }

    pub fn inputs(&self) -> Vec<PsetInput> {
        self.inner.inputs().iter().map(Into::into).collect()
    }
}

/// PSET input
#[wasm_bindgen]
pub struct PsetInput {
    inner: Input,
}

impl From<&Input> for PsetInput {
    fn from(inner: &Input) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

#[wasm_bindgen]
impl PsetInput {
    /// Prevout TXID of the input
    pub fn previous_txid(&self) -> Txid {
        self.inner.previous_txid.into()
    }

    /// Prevout vout of the input
    pub fn previous_vout(&self) -> u32 {
        self.inner.previous_output_index
    }

    /// If the input has an issuance, the asset id
    #[wasm_bindgen(js_name = issuanceAsset)]
    pub fn issuance_asset(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().0.into())
    }

    /// If the input has an issuance, the token id
    #[wasm_bindgen(js_name = issuanceToken)]
    pub fn issuance_token(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().1.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::Pset;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn pset_roundtrip() {
        let pset_string =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let tx_expected =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed_transaction.hex").to_string();
        let tx_string = pset.extract_tx().unwrap().to_string();
        assert_eq!(tx_expected, tx_string);

        assert_eq!(pset_string, pset.to_string());

        let pset_in = &pset.inputs()[0];
        assert_eq!(
            pset_in.previous_txid().to_string(),
            "0093c96a69e9ea00b5409611f23435b6639c157afa1c88cf18960715ea10116c"
        );
        assert_eq!(pset_in.previous_vout(), 0);

        assert!(pset_in.issuance_asset().is_none());
        assert!(pset_in.issuance_token().is_none());
    }
}
