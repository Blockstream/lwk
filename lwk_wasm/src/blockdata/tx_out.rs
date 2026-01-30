//! Liquid transaction output

use crate::{Address, AssetId, Error, Network, Script, SecretKey, TxOutSecrets};

use lwk_wollet::elements::{self, confidential, TxOutWitness};

use wasm_bindgen::prelude::*;

/// A transaction output
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TxOut {
    inner: elements::TxOut,
}

impl From<elements::TxOut> for TxOut {
    fn from(inner: elements::TxOut) -> Self {
        Self { inner }
    }
}

impl From<TxOut> for elements::TxOut {
    fn from(value: TxOut) -> Self {
        value.inner
    }
}

impl From<&TxOut> for elements::TxOut {
    fn from(value: &TxOut) -> Self {
        value.inner.clone()
    }
}

impl AsRef<elements::TxOut> for TxOut {
    fn as_ref(&self) -> &elements::TxOut {
        &self.inner
    }
}

#[wasm_bindgen]
impl TxOut {
    /// Create a TxOut with explicit asset and value from script pubkey and asset ID.
    ///
    /// This is useful for constructing UTXOs for Simplicity transaction signing.
    #[wasm_bindgen(js_name = fromExplicit)]
    pub fn from_explicit(script_pubkey: &Script, asset_id: &AssetId, satoshi: u64) -> TxOut {
        let inner = elements::TxOut {
            script_pubkey: script_pubkey.as_ref().clone(),
            asset: confidential::Asset::Explicit((*asset_id).into()),
            value: confidential::Value::Explicit(satoshi),
            nonce: confidential::Nonce::Null,
            witness: TxOutWitness::default(),
        };
        Self { inner }
    }

    /// Get the scriptpubkey
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey.clone().into()
    }

    /// Whether or not this output is a fee output
    #[wasm_bindgen(js_name = isFee)]
    pub fn is_fee(&self) -> bool {
        self.inner.is_fee()
    }

    /// Returns if at least some part of this output is blinded
    #[wasm_bindgen(js_name = isPartiallyBlinded)]
    pub fn is_partially_blinded(&self) -> bool {
        self.inner.is_partially_blinded()
    }

    /// If explicit returns the asset, if confidential returns undefined
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset.explicit().map(Into::into)
    }

    /// If explicit returns the value, if confidential returns undefined
    pub fn value(&self) -> Option<u64> {
        self.inner.value.explicit()
    }

    /// Get the unconfidential address for this output
    #[wasm_bindgen(js_name = unconfidentialAddress)]
    pub fn unconfidential_address(&self, network: &Network) -> Option<Address> {
        let params = lwk_wollet::ElementsNetwork::from(network).address_params();
        elements::Address::from_script(&self.inner.script_pubkey, None, params).map(|a| a.into())
    }

    /// Unblind the output using the given secret key
    pub fn unblind(&self, secret_key: &SecretKey) -> Result<TxOutSecrets, Error> {
        Ok(self
            .inner
            .unblind(&lwk_wollet::EC, secret_key.into())
            .map(Into::into)?)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::TxOut;
    use crate::{AssetId, Network, Script};
    use lwk_wollet::elements::{self, confidential};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tx_out() {
        let asset_hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset_id = AssetId::new(asset_hex).unwrap();

        let script = Script::new("0014e8df018c7e326cc253faac7e46cdc51e68542c42").unwrap();
        let tx_out = TxOut::from_explicit(&script, &asset_id, 1000);
        assert!(!tx_out.is_fee());
        assert!(!tx_out.is_partially_blinded());
        assert_eq!(tx_out.value(), Some(1000));
        assert_eq!(tx_out.asset().unwrap().to_string(), asset_hex);
        assert!(tx_out
            .unconfidential_address(&Network::regtest_default())
            .is_some());

        let fee_output = elements::TxOut {
            script_pubkey: elements::Script::new(),
            asset: confidential::Asset::Explicit(asset_hex.parse().unwrap()),
            value: confidential::Value::Explicit(250),
            nonce: confidential::Nonce::Null,
            witness: elements::TxOutWitness::default(),
        };
        let fee_tx_out: TxOut = fee_output.into();
        assert!(fee_tx_out.is_fee());
        assert_eq!(fee_tx_out.value(), Some(250));
    }
}
