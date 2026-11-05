//! The transaction a builder produced, with the data that cannot live in the PSET

use std::{
    collections::{BTreeMap, HashMap},
    vec,
};

use crate::{DownloadTxResult, Error, Update, Wollet, EC};
use elements::{pset::PartiallySignedTransaction, OutPoint, TxOutSecrets};

/// Transaction with metadata
///
/// **Experimental**: this API might change without notice.
///
/// A PSET and other data that cannot be in the PSET
pub struct BuiltTx {
    pub(super) pset: PartiallySignedTransaction,
    // type is what PartiallySignedTransaction.blind_last() returns
    pub(super) blind_secrets: BTreeMap<
        elements::CtLocation,
        (
            elements::confidential::AssetBlindingFactor,
            elements::confidential::ValueBlindingFactor,
            elements::secp256k1_zkp::SecretKey,
        ),
    >,
}

impl BuiltTx {
    /// Pset
    pub fn pset(&self) -> &PartiallySignedTransaction {
        &self.pset
    }

    /// Blinding nonces
    pub fn blinding_nonces(&self) -> Result<Vec<String>, Error> {
        let mut m = HashMap::new();
        for (ct_location, (_abf, _vbf, eph_sk)) in self.blind_secrets.iter() {
            // these are outputs not inputs...
            if let elements::CtLocation {
                input_index,
                ty: elements::CtLocationType::Input,
            } = ct_location
            {
                m.insert(input_index, eph_sk);
            }
        }

        let mut blinding_nonces = vec![];
        for idx in 0..self.pset.n_outputs() {
            let bn = if let Some(eph_sk) = m.get(&idx) {
                let blinding_pubkey = self.pset.outputs()[idx]
                    .blinding_key
                    .ok_or_else(|| Error::Generic("Missing blinding key".into()))?;
                let (_nonce, shared_secret) = elements::confidential::Nonce::with_ephemeral_sk(
                    &EC,
                    **eph_sk,
                    &blinding_pubkey.inner,
                );
                shared_secret.display_secret().to_string()
            } else {
                "".to_string()
            };
            blinding_nonces.push(bn);
        }
        Ok(blinding_nonces)
    }

    /// Convert to `Amp0Pset`
    #[cfg(feature = "amp0")]
    pub fn to_amp0(self) -> Result<crate::amp0::Amp0Pset, Error> {
        let blinding_nonces = self.blinding_nonces()?;
        let BuiltTx { pset, .. } = self;
        crate::amp0::Amp0Pset::new(pset, blinding_nonces)
    }

    /// Secrets of the outputs blinded by the wallet
    ///
    /// The returned vector has one element per PSET output, in the same order.
    /// An element is `None` if the output was not blinded by this builder, for example the fee
    /// output, an explicit output or an output of an unblinded transaction.
    pub fn output_secrets(&self) -> Result<Vec<Option<TxOutSecrets>>, Error> {
        let mut secrets = vec![None; self.pset.n_outputs()];
        for (ct_location, (abf, vbf, _eph_sk)) in self.blind_secrets.iter() {
            // these are outputs not inputs...
            if let elements::CtLocation {
                input_index: vout,
                ty: elements::CtLocationType::Input,
            } = ct_location
            {
                let output = self
                    .pset
                    .outputs()
                    .get(*vout)
                    .ok_or_else(|| Error::Generic("Missing output".into()))?;
                let asset = output
                    .asset
                    .ok_or_else(|| Error::Generic("Missing asset".into()))?;
                let value = output
                    .amount
                    .ok_or_else(|| Error::Generic("Missing amount".into()))?;
                secrets[*vout] = Some(TxOutSecrets::new(asset, *abf, value, *vbf));
            }
        }
        Ok(secrets)
    }

    fn unblinded(&self) -> Result<Vec<(OutPoint, TxOutSecrets)>, Error> {
        // consider getting txid from global.unsigned_tx if extract_tx errors
        let txid = self.pset.extract_tx()?.txid();
        let unblinded = self
            .output_secrets()?
            .into_iter()
            .enumerate()
            .filter_map(|(vout, secrets)| {
                secrets.map(|secrets| (OutPoint::new(txid, vout as u32), secrets))
            })
            .collect();
        Ok(unblinded)
    }

    /// Update to persist the blinder
    pub fn update(&self, wollet: &Wollet) -> Result<Update, Error> {
        let unblinds = self.unblinded()?;
        let update = Update {
            version: 4,
            wollet_status: wollet.status(),
            new_txs: DownloadTxResult {
                txs: vec![],
                unblinds,
            },
            txid_height_new: vec![],
            txid_height_delete: vec![],
            timestamps: vec![],
            scripts_with_blinding_pubkey: vec![],
            tip: crate::update::default_blockheader(),
            unspent: vec![],
            last_unused: crate::wollet::WolletState::last_unused(wollet),
        };
        Ok(update)
    }
}
