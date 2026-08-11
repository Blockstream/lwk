//! Attaches silent-payment signing metadata to PSET inputs.

use std::collections::HashMap;

use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::{OutPoint, Script};
use crate::error::Error;
use crate::silentpayments::{SilentPaymentScanMaterial, SilentPaymentUtxo, SpendTweak};
use crate::wollet::Wollet;

/// Attaches silent-payment metadata to PSET inputs.
pub(crate) struct SilentPaymentPsetAnnotator<'a> {
    wollet: &'a Wollet,

    material: &'a SilentPaymentScanMaterial,

    /// Standalone output tweaks by outpoint.
    standalone: HashMap<OutPoint, SpendTweak>,
}

impl<'a> SilentPaymentPsetAnnotator<'a> {
    /// Creates an annotator for the wallet and standalone silent-payment UTXOs.
    pub(crate) fn for_builder(
        wollet: &'a Wollet,
        utxos: &[SilentPaymentUtxo],
    ) -> Result<Option<Self>, Error> {
        let material = match wollet.silent_payment_material() {
            Some(material) => material,
            None if utxos.is_empty() => return Ok(None),
            None => return Err(Error::MissingSilentPaymentKeys),
        };

        Ok(Some(SilentPaymentPsetAnnotator {
            wollet,
            material,
            standalone: utxos.iter().map(|u| (u.outpoint, u.spend_tweak)).collect(),
        }))
    }

    /// Annotates silent-payment inputs, leaving ordinary inputs alone.
    pub(crate) fn annotate(&self, pset: &mut PartiallySignedTransaction) -> Result<(), Error> {
        for idx in 0..pset.inputs().len() {
            let input = &pset.inputs()[idx];
            let outpoint = OutPoint::new(input.previous_txid, input.previous_output_index);
            let script = input
                .witness_utxo
                .as_ref()
                .map(|txout| txout.script_pubkey.clone());

            let Some(tweak) = self.tweak_for(&outpoint, script.as_ref()) else {
                continue; // an ordinary descriptor-derived input
            };

            // Reject metadata that does not match the selected output.
            self.verify(&outpoint, script.as_ref(), &tweak)?;

            let input = pset
                .inputs_mut()
                .get_mut(idx)
                .ok_or_else(|| Error::MissingVin)?;
            self.material.input_meta(*tweak.as_scalar()).attach(input);
        }
        Ok(())
    }

    /// Returns the standalone or cached spend tweak.
    fn tweak_for(&self, outpoint: &OutPoint, script: Option<&Script>) -> Option<SpendTweak> {
        if let Some(tweak) = self.standalone.get(outpoint) {
            return Some(*tweak);
        }
        self.wollet
            .cache
            .silent_payment(script?)
            .map(|entry| entry.spend_tweak)
    }

    /// Checks `B_spend + tweak·G == x_only(P_k)` against the witness UTXO.
    fn verify(
        &self,
        outpoint: &OutPoint,
        script: Option<&Script>,
        tweak: &SpendTweak,
    ) -> Result<(), Error> {
        let Some(script) = script else {
            return Err(Error::Generic(format!(
                "silent payment input {outpoint} has no witness utxo to verify its tweak against"
            )));
        };
        let verified = tweak
            .applied_to(&self.material.spend_pubkey())
            .map(|expected| Self::output_key(script) == Some(expected.x_only_public_key().0))
            .unwrap_or(false);
        if verified {
            Ok(())
        } else {
            Err(Error::Generic(format!(
                "silent payment {outpoint} does not verify against this wallet's spend key"
            )))
        }
    }

    /// The x-only output key of a v1 Taproot program, or `None` if `script` is not one.
    fn output_key(script: &Script) -> Option<crate::elements::secp256k1_zkp::XOnlyPublicKey> {
        let bytes = script.as_bytes();
        // `OP_1 <32-byte push>`: 0x51 0x20 followed by the key.
        if bytes.len() != 34 || bytes[0] != 0x51 || bytes[1] != 0x20 {
            return None;
        }
        crate::elements::secp256k1_zkp::XOnlyPublicKey::from_slice(&bytes[2..]).ok()
    }
}
