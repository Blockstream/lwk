//! Liquid transaction output

use crate::{Address, AssetId, Error, Network, Script, SecretKey, Transaction, TxOutSecrets};

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
    /// Extract a TxOut from a transaction at the given output index.
    ///
    /// Returns the TxOut with its witness data stripped: PSET carries the rangeproof
    /// in a dedicated input field (`in_utxo_rangeproof`), and the surjection proof is
    /// consumed during prior-tx verification and not re-carried.
    ///
    /// Use this to get the real (possibly confidential) `witness_utxo` for PSET
    /// construction. The returned TxOut is NOT suitable for unblinding because the
    /// rangeproof is stripped — to unblind, take the output directly from the source
    /// transaction.
    #[wasm_bindgen(js_name = fromTransaction)]
    pub fn from_transaction(tx: &Transaction, vout: u32) -> Result<TxOut, Error> {
        let tx_ref: &elements::Transaction = tx.as_ref();
        let txout = tx_ref.output.get(vout as usize).ok_or_else(|| {
            Error::Generic(format!(
                "vout {} out of range (tx has {} outputs)",
                vout,
                tx_ref.output.len()
            ))
        })?;
        // Strip witness data — PSET stores rangeproof in a separate field.
        let mut clean = txout.clone();
        clean.witness = TxOutWitness::default();
        Ok(Self { inner: clean })
    }

    /// Extract the rangeproof bytes from a transaction output's witness.
    ///
    /// Returns the rangeproof bytes, or `None` if the vout is out of range or the
    /// output is explicit (no rangeproof). Use with
    /// `PsetInputBuilder.inUtxoRangeproof()`.
    #[wasm_bindgen(js_name = rangeproofFromTransaction)]
    pub fn rangeproof_from_transaction(tx: &Transaction, vout: u32) -> Option<Vec<u8>> {
        let tx_ref: &elements::Transaction = tx.as_ref();
        let txout = tx_ref.output.get(vout as usize)?;
        txout.witness.rangeproof.as_ref().map(|rp| rp.serialize())
    }

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
        let params = lwk_common::Network::from(network).address_params();
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
        let asset_id = AssetId::from_string(asset_hex).unwrap();

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

    #[wasm_bindgen_test]
    fn test_from_transaction() {
        use crate::Transaction;

        let tx_hex = include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
            .trim_end();
        let tx = Transaction::new(tx_hex).unwrap();
        let tx_ref: &elements::Transaction = tx.as_ref();
        let outputs_len = tx_ref.output.len() as u32;
        assert!(outputs_len > 0, "fixture has at least one output");

        // First output is confidential in the fixture; witness must be stripped.
        let extracted = TxOut::from_transaction(&tx, 0).unwrap();
        let inner: elements::TxOut = (&extracted).into();
        assert_eq!(inner.witness, elements::TxOutWitness::default());

        // Out-of-range vout returns an error.
        assert!(TxOut::from_transaction(&tx, outputs_len).is_err());

        // rangeproof_from_transaction returns Some for confidential outputs.
        let rp = TxOut::rangeproof_from_transaction(&tx, 0);
        assert!(rp.is_some(), "confidential output should have a rangeproof");

        // The last output of a Liquid tx is the explicit fee — no rangeproof.
        assert!(
            TxOut::rangeproof_from_transaction(&tx, outputs_len - 1).is_none(),
            "fee output is explicit and should have no rangeproof"
        );

        // Out-of-range silently returns None.
        assert!(TxOut::rangeproof_from_transaction(&tx, outputs_len).is_none());
    }
}
