use crate::Error;

use lwk_wollet::elements;
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;

use wasm_bindgen::prelude::*;

/// A transaction input witness.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TxInWitness {
    inner: elements::TxInWitness,
}

impl From<elements::TxInWitness> for TxInWitness {
    fn from(inner: elements::TxInWitness) -> Self {
        Self { inner }
    }
}

impl From<TxInWitness> for elements::TxInWitness {
    fn from(value: TxInWitness) -> Self {
        value.inner
    }
}

impl From<&TxInWitness> for elements::TxInWitness {
    fn from(value: &TxInWitness) -> Self {
        value.inner.clone()
    }
}

impl AsRef<elements::TxInWitness> for TxInWitness {
    fn as_ref(&self) -> &elements::TxInWitness {
        &self.inner
    }
}

#[wasm_bindgen]
impl TxInWitness {
    /// Create an empty witness.
    pub fn empty() -> TxInWitness {
        Self {
            inner: elements::TxInWitness::default(),
        }
    }

    /// Create a witness from script witness elements.
    ///
    /// Takes an array of hex strings representing the witness stack.
    #[wasm_bindgen(js_name = fromScriptWitness)]
    pub fn from_script_witness(script_witness: Vec<String>) -> Result<TxInWitness, Error> {
        let witness: Vec<Vec<u8>> = script_witness
            .iter()
            .map(|s| Vec::<u8>::from_hex(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inner: elements::TxInWitness {
                script_witness: witness,
                ..Default::default()
            },
        })
    }

    /// Get the script witness elements.
    ///
    /// Returns an array of hex strings.
    #[wasm_bindgen(js_name = scriptWitness)]
    pub fn script_witness(&self) -> Vec<String> {
        self.inner
            .script_witness
            .iter()
            .map(|elem| elem.to_hex())
            .collect()
    }

    /// Get the peg-in witness elements.
    ///
    /// Returns an array of hex strings.
    #[wasm_bindgen(js_name = peginWitness)]
    pub fn pegin_witness(&self) -> Vec<String> {
        self.inner
            .pegin_witness
            .iter()
            .map(|elem| elem.to_hex())
            .collect()
    }

    /// Check if the witness is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Builder for creating a TxInWitness.
#[wasm_bindgen]
pub struct TxInWitnessBuilder {
    inner: elements::TxInWitness,
}

#[wasm_bindgen]
impl TxInWitnessBuilder {
    /// Create a new witness builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> TxInWitnessBuilder {
        Self {
            inner: elements::TxInWitness::default(),
        }
    }

    /// Set the script witness elements.
    ///
    /// Takes an array of hex strings representing the witness stack.
    #[wasm_bindgen(js_name = scriptWitness)]
    pub fn script_witness(mut self, witness: Vec<String>) -> Result<TxInWitnessBuilder, Error> {
        self.inner.script_witness = witness
            .iter()
            .map(|s| Vec::<u8>::from_hex(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Set the peg-in witness elements.
    ///
    /// Takes an array of hex strings representing the peg-in witness stack.
    #[wasm_bindgen(js_name = peginWitness)]
    pub fn pegin_witness(mut self, witness: Vec<String>) -> Result<TxInWitnessBuilder, Error> {
        self.inner.pegin_witness = witness
            .iter()
            .map(|s| Vec::<u8>::from_hex(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Set the amount rangeproof from serialized bytes.
    #[wasm_bindgen(js_name = amountRangeproof)]
    pub fn amount_rangeproof(mut self, proof: &[u8]) -> Result<TxInWitnessBuilder, Error> {
        let rangeproof = elements::secp256k1_zkp::RangeProof::from_slice(proof)?;
        self.inner.amount_rangeproof = Some(Box::new(rangeproof));
        Ok(self)
    }

    /// Set the inflation keys rangeproof from serialized bytes.
    #[wasm_bindgen(js_name = inflationKeysRangeproof)]
    pub fn inflation_keys_rangeproof(mut self, proof: &[u8]) -> Result<TxInWitnessBuilder, Error> {
        let rangeproof = elements::secp256k1_zkp::RangeProof::from_slice(proof)?;
        self.inner.inflation_keys_rangeproof = Some(Box::new(rangeproof));
        Ok(self)
    }

    /// Build the TxInWitness.
    pub fn build(self) -> TxInWitness {
        TxInWitness::from(self.inner)
    }
}

impl Default for TxInWitnessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{TxInWitness, TxInWitnessBuilder};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tx_in_witness() {
        let empty = TxInWitness::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.script_witness().len(), 0);

        let witness_stack = vec!["010203".to_string(), "040506".to_string()];

        let witness = TxInWitness::from_script_witness(witness_stack).unwrap();
        assert!(!witness.is_empty());
        assert_eq!(witness.script_witness().len(), 2);
        assert_eq!(witness.script_witness()[0], "010203");
        assert_eq!(witness.script_witness()[1], "040506");
    }

    #[wasm_bindgen_test]
    fn test_tx_in_witness_builder() {
        let witness_stack = vec!["010203".to_string()];

        let witness = TxInWitnessBuilder::new()
            .script_witness(witness_stack)
            .unwrap()
            .build();

        assert!(!witness.is_empty());
        assert_eq!(witness.script_witness().len(), 1);
    }
}
