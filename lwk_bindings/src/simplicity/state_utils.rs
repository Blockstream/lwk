//! NOTE: The main purpose of this binding is to allow users to try it.
//! Eventually, when we have a better understanding, this will be refactored.
//!
//! References for state management can be found here:
//! <https://github.com/BlockstreamResearch/simplicity-contracts/tree/main/crates/contracts/src>

use crate::types::{ControlBlock, XOnlyPublicKey};
use crate::{Cmr, LwkError, Script};

use std::sync::Arc;

use lwk_wollet::elements::taproot;
use lwk_wollet::elements_miniscript::ToPublicKey;
use lwk_wollet::hashes::{sha256, Hash};
use lwk_wollet::{elements, EC};

use lwk_simplicity::scripts::{simplicity_leaf_version, tap_data_hash};
use lwk_simplicity::simplicityhl;

/// Taproot builder for Simplicity-related functionality.
///
/// This builder is tailored for state-management trees that use Taproot. It
/// is not intended to be a fully general-purpose Taproot construction API.
#[derive(uniffi::Object, Clone, Debug)]
pub struct StateTaprootBuilder {
    // This builder does not use a `Mutex`. `add_leaf_with_ver` consumes `self`,
    // so we clone and return a new builder on each update.
    inner: taproot::TaprootBuilder,
}

#[uniffi::export]
impl StateTaprootBuilder {
    /// Create a new builder.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: taproot::TaprootBuilder::new(),
        })
    }

    /// Add a Simplicity leaf at `depth`.
    pub fn add_simplicity_leaf(&self, depth: u8, cmr: &Cmr) -> Result<Arc<Self>, LwkError> {
        let (script, version) = script_version(cmr.inner());
        let inner = self
            .inner
            .clone()
            .add_leaf_with_ver(usize::from(depth), script, version)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Add a TapData hidden leaf at `depth`.
    pub fn add_data_leaf(&self, depth: u8, data: &[u8]) -> Result<Arc<Self>, LwkError> {
        let hash = tap_data_hash(data);
        let inner = self.inner.clone().add_hidden(usize::from(depth), hash)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Add a precomputed hidden hash at `depth`.
    pub fn add_hidden_hash(&self, depth: u8, hash: &[u8]) -> Result<Arc<Self>, LwkError> {
        let hash: [u8; 32] = hash.try_into()?;
        let hash = sha256::Hash::from_byte_array(hash);
        let inner = self.inner.clone().add_hidden(usize::from(depth), hash)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Finalize and produce Taproot spend info.
    pub fn finalize(
        &self,
        internal_key: &XOnlyPublicKey,
    ) -> Result<Arc<StateTaprootSpendInfo>, LwkError> {
        let x_only_key = internal_key.to_simplicityhl()?;
        let spend_info = self.inner.clone().finalize(&EC, x_only_key)?;
        Ok(Arc::new(StateTaprootSpendInfo { inner: spend_info }))
    }
}

/// Taproot spending information.
#[derive(uniffi::Object)]
pub struct StateTaprootSpendInfo {
    pub(crate) inner: taproot::TaprootSpendInfo,
}

#[uniffi::export]
impl StateTaprootSpendInfo {
    /// Get the tweaked Taproot output key.
    pub fn output_key(&self) -> Arc<XOnlyPublicKey> {
        Arc::new(XOnlyPublicKey::from(self.inner.output_key().into_inner()))
    }

    /// Get output key parity (0 for even, 1 for odd).
    pub fn output_key_parity(&self) -> u8 {
        self.inner.output_key_parity().to_u8()
    }

    /// Get the internal key.
    pub fn internal_key(&self) -> Arc<XOnlyPublicKey> {
        Arc::new(XOnlyPublicKey::from(
            self.inner.internal_key().to_x_only_pubkey(),
        ))
    }

    /// Get the Taproot script tree merkle root bytes, if present.
    pub fn merkle_root(&self) -> Option<Vec<u8>> {
        self.inner
            .merkle_root()
            .map(|root| root.to_byte_array().to_vec())
    }

    /// Get the control block for a script identified by CMR.
    pub fn control_block(&self, cmr: &Cmr) -> Result<Arc<ControlBlock>, LwkError> {
        let control_block = self
            .inner
            .control_block(&script_version(cmr.inner()))
            .ok_or_else(|| LwkError::Generic {
                msg: "CMR is not part of this taproot spend info".into(),
            })?;
        Ok(Arc::new(control_block.try_into()?))
    }

    /// Get script pubkey as v1 P2TR output script for the tweaked output key.
    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(elements::Script::new_v1_p2tr_tweaked(self.inner.output_key()).into())
    }
}

fn script_version(cmr: simplicityhl::simplicity::Cmr) -> (elements::Script, taproot::LeafVersion) {
    let script = elements::Script::from(cmr.as_ref().to_vec());
    (script, simplicity_leaf_version())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_state_management_flow() {
        let mut state: [u8; 32] = [0u8; 32];
        state[31] = 1;

        let cmr = Cmr::from_hex(
            crate::types::Hex::from_str(
                "cbd8d3d0cc95384237c1bf20334c30b579f22058563c37731a3ab2bc76d5a248",
            )
            .unwrap(),
        )
        .unwrap();
        let internal_key = XOnlyPublicKey::from_str(
            "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
        )
        .unwrap();

        let expected_script_pubkey =
            "51205920ca2ef73fa8c0378b50e99e4518b72fee1c413c1f1c52acbde479b3ec0a21".to_string();

        let builder = StateTaprootBuilder::new()
            .add_simplicity_leaf(1, &cmr)
            .unwrap()
            .add_data_leaf(1, state.as_slice())
            .unwrap()
            .finalize(&internal_key)
            .unwrap();

        assert_eq!(builder.script_pubkey().to_string(), expected_script_pubkey);
    }
}
