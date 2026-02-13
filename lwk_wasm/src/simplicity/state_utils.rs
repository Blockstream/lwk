//! NOTE: The main purpose of this binding is to allow users to try it.
//! Eventually, when we have a better understanding, this will be refactored.
//!
//! References for state management can be found here:
//! <https://github.com/BlockstreamResearch/simplicity-contracts/tree/main/crates/contracts/src>

use crate::{ControlBlock, Error, Script, XOnlyPublicKey};

use super::cmr::Cmr;

use lwk_wollet::elements::taproot;
use lwk_wollet::elements_miniscript::ToPublicKey;
use lwk_wollet::hashes::{sha256, Hash};
use lwk_wollet::{elements, EC};

use lwk_simplicity::scripts::{simplicity_leaf_version, tap_data_hash};

use lwk_simplicity::simplicityhl;
use wasm_bindgen::prelude::*;

/// Taproot builder for Simplicity-related functionality.
///
/// This builder is tailored for state-management trees that combine a Simplicity
/// leaf with hidden TapData leaves, but it can also be used for generic trees.
#[wasm_bindgen]
pub struct StateTaprootBuilder {
    inner: taproot::TaprootBuilder,
}

impl Default for StateTaprootBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl StateTaprootBuilder {
    /// Create a new builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> StateTaprootBuilder {
        StateTaprootBuilder {
            inner: taproot::TaprootBuilder::new(),
        }
    }

    /// Add a Simplicity leaf at `depth`.
    #[wasm_bindgen(js_name = addSimplicityLeaf)]
    pub fn add_simplicity_leaf(&self, depth: u8, cmr: &Cmr) -> Result<StateTaprootBuilder, Error> {
        let (script, version) = script_version(cmr.inner());
        let inner = self
            .inner
            .clone()
            .add_leaf_with_ver(usize::from(depth), script, version)?;
        Ok(StateTaprootBuilder { inner })
    }

    /// Add a TapData hidden leaf at `depth`.
    #[wasm_bindgen(js_name = addDataLeaf)]
    pub fn add_data_leaf(&self, depth: u8, data: &[u8]) -> Result<StateTaprootBuilder, Error> {
        let hash = tap_data_hash(data);
        let inner = self.inner.clone().add_hidden(usize::from(depth), hash)?;
        Ok(StateTaprootBuilder { inner })
    }

    /// Add a precomputed hidden hash at `depth`.
    #[wasm_bindgen(js_name = addHiddenHash)]
    pub fn add_hidden_hash(&self, depth: u8, hash: &[u8]) -> Result<StateTaprootBuilder, Error> {
        let hash: [u8; 32] = hash.try_into().map_err(|_| {
            Error::Generic(format!("hidden hash must be 32 bytes, got {}", hash.len()))
        })?;
        let hash = sha256::Hash::from_byte_array(hash);
        let inner = self.inner.clone().add_hidden(usize::from(depth), hash)?;
        Ok(StateTaprootBuilder { inner })
    }

    /// Finalize and produce Taproot spend info.
    pub fn finalize(&self, internal_key: &XOnlyPublicKey) -> Result<StateTaprootSpendInfo, Error> {
        let x_only_key = internal_key.to_simplicityhl()?;
        let spend_info = self.inner.clone().finalize(&EC, x_only_key)?;
        Ok(StateTaprootSpendInfo { inner: spend_info })
    }
}

/// Taproot spending information.
#[wasm_bindgen]
pub struct StateTaprootSpendInfo {
    pub(crate) inner: taproot::TaprootSpendInfo,
}

#[wasm_bindgen]
impl StateTaprootSpendInfo {
    /// Get the tweaked Taproot output key.
    #[wasm_bindgen(js_name = outputKey)]
    pub fn output_key(&self) -> XOnlyPublicKey {
        XOnlyPublicKey::from(self.inner.output_key().into_inner())
    }

    /// Get output key parity (0 for even, 1 for odd).
    #[wasm_bindgen(js_name = outputKeyParity)]
    pub fn output_key_parity(&self) -> u8 {
        self.inner.output_key_parity().to_u8()
    }

    /// Get the internal key.
    #[wasm_bindgen(js_name = internalKey)]
    pub fn internal_key(&self) -> XOnlyPublicKey {
        XOnlyPublicKey::from(self.inner.internal_key().to_x_only_pubkey())
    }

    /// Get the Taproot script tree merkle root bytes, if present.
    #[wasm_bindgen(js_name = merkleRoot)]
    pub fn merkle_root(&self) -> Option<Vec<u8>> {
        self.inner
            .merkle_root()
            .map(|root| root.to_byte_array().to_vec())
    }

    /// Get the control block for a script identified by CMR.
    #[wasm_bindgen(js_name = controlBlock)]
    pub fn control_block(&self, cmr: &Cmr) -> Result<ControlBlock, Error> {
        let control_block = self
            .inner
            .control_block(&script_version(cmr.inner()))
            .ok_or_else(|| Error::Generic("CMR is not part of this taproot spend info".into()))?;
        Ok(control_block.try_into()?)
    }

    /// Get script pubkey as v1 P2TR output script for the tweaked output key.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        elements::Script::new_v1_p2tr_tweaked(self.inner.output_key()).into()
    }
}

fn script_version(cmr: simplicityhl::simplicity::Cmr) -> (elements::Script, taproot::LeafVersion) {
    let script = elements::Script::from(cmr.as_ref().to_vec());
    (script, simplicity_leaf_version())
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_state_management_flow() {
        let mut state = [0u8; 32];
        state[31] = 1;

        let cmr = Cmr::from_hex("cbd8d3d0cc95384237c1bf20334c30b579f22058563c37731a3ab2bc76d5a248")
            .unwrap();
        let internal_key =
            XOnlyPublicKey::new("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0")
                .unwrap();

        let spend_info = StateTaprootBuilder::new()
            .add_simplicity_leaf(1, &cmr)
            .unwrap()
            .add_data_leaf(1, state.as_slice())
            .unwrap()
            .finalize(&internal_key)
            .unwrap();

        assert_eq!(
            spend_info.script_pubkey().to_string(),
            "51205920ca2ef73fa8c0378b50e99e4518b72fee1c413c1f1c52acbde479b3ec0a21"
        );
    }
}
