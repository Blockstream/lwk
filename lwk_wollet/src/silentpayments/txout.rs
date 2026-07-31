//! Building the confidential `TxOut` that pays a silent payment output.

use crate::silentpayments::SilentPaymentOutput;
use crate::util::EC;
use crate::Error;

/// Builds silent-payment confidential outputs without surjection proofs.
pub struct SpTxOutBuilder;

impl SpTxOutBuilder {
    /// The rangeproof exponent and minimum-bits, matching `TxBuilder`'s blinding.
    const RANGEPROOF_EXP: i32 = 0;
    const RANGEPROOF_MIN_BITS: u8 = 52;

    /// Builds a blinded output and its blinding secrets.
    pub fn build(
        out: &SilentPaymentOutput,
        asset: crate::elements::AssetId,
        value: u64,
        rng: &mut (impl rand::RngCore + rand::CryptoRng),
    ) -> Result<(crate::elements::TxOut, crate::elements::TxOutSecrets), Error> {
        use crate::elements::confidential::{
            Asset, AssetBlindingFactor, Nonce, Value, ValueBlindingFactor,
        };
        use crate::elements::secp256k1_zkp::{Generator, PedersenCommitment, RangeProof, Tag};
        use crate::elements::{TxOut, TxOutSecrets, TxOutWitness};

        let script_pubkey = out.script_pubkey();
        let abf = AssetBlindingFactor::new(&mut *rng);
        let vbf = ValueBlindingFactor::new(&mut *rng);

        let (nonce, ct_shared_secret) =
            Nonce::new_confidential(&mut *rng, &EC, &out.blinding_pubkey);

        let asset_tag = Tag::from(asset.into_inner().to_byte_array());
        let asset_generator = Generator::new_blinded(&EC, asset_tag, abf.into_inner());
        let value_commitment =
            PedersenCommitment::new(&EC, value, vbf.into_inner(), asset_generator);

        let mut message = [0u8; 64];
        message[..32].copy_from_slice(&asset.into_inner().to_byte_array());
        message[32..].copy_from_slice(abf.into_inner().as_ref());

        let rangeproof = RangeProof::new(
            &EC,
            1,
            value_commitment,
            value,
            vbf.into_inner(),
            &message,
            script_pubkey.as_bytes(),
            ct_shared_secret,
            Self::RANGEPROOF_EXP,
            Self::RANGEPROOF_MIN_BITS,
            asset_generator,
        )?;

        let txout = TxOut {
            asset: Asset::Confidential(asset_generator),
            value: Value::Confidential(value_commitment),
            nonce,
            script_pubkey,
            witness: TxOutWitness {
                surjection_proof: None,
                rangeproof: Some(Box::new(rangeproof)),
            },
        };
        let secrets = TxOutSecrets {
            asset,
            asset_bf: abf,
            value,
            value_bf: vbf,
        };
        Ok((txout, secrets))
    }
}
