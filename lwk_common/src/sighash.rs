//! Shared PSET sighash computation.

use elements::{
    hashes::Hash,
    pset::{Input, PartiallySignedTransaction},
    sighash::{Prevouts, SighashCache},
    taproot::{TapLeafHash, TapSighashHash},
    BlockHash, Sighash, Transaction,
};
use elements_miniscript::psbt::{PsbtExt, PsbtSighashMsg};

use crate::input::{is_taproot_input, spent_txout};

/// Elements Core masks this flag from the in-memory outpoint of a pegin input.
const PEGIN_FLAG: u32 = 1 << 30;

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum SighashError {
    #[error("transaction extraction from PSET failed")]
    TxExtraction,

    #[error("input #{0} does not exist")]
    IndexOutOfBounds(usize),

    #[error("input #{0}: the PSET does not carry the spent output")]
    MissingSpentOutput(usize),

    #[error("input #{0}: expected a taproot input")]
    NotTaproot(usize),

    #[error("input #{0}: expected a non-taproot input")]
    UnexpectedTaproot(usize),

    #[error("input #{0}: the declared sighash type is not a valid schnorr one")]
    InvalidSchnorrSighashType(usize),

    #[error("taproot input requires the ELIP-101 genesis hash, absent from the PSET")]
    MissingGenesisHash,

    #[error(transparent)]
    Computation(#[from] elements::sighash::Error),

    #[error(transparent)]
    Psbt(#[from] elements_miniscript::psbt::SighashError),
}

/// Computes the messages the inputs of a PSET commit to.
///
/// **Experimental**: this API might change without notice.
pub struct SighashCtx<'a> {
    pset: &'a PartiallySignedTransaction,
    cache: SighashCache<Box<Transaction>>,
    genesis_hash: Option<BlockHash>,
}

impl<'a> SighashCtx<'a> {
    /// Build a context over a PSET
    ///
    /// **Experimental**: this API might change without notice.
    pub fn new(
        pset: &'a PartiallySignedTransaction,
        genesis_hash: Option<BlockHash>,
    ) -> Result<Self, SighashError> {
        let mut tx = pset.extract_tx().map_err(|_| SighashError::TxExtraction)?;
        for input in &mut tx.input {
            if input.is_pegin() {
                // TODO: Remove once https://github.com/ElementsProject/rust-elements/issues/292
                // is fixed. Elements Core masks this flag from the in-memory outpoint.
                input.previous_output.vout &= !PEGIN_FLAG;
            }
        }

        Ok(Self {
            pset,
            cache: SighashCache::new(Box::new(tx)),
            genesis_hash,
        })
    }

    /// The ECDSA message, with the script code inferred from the input.
    ///
    /// **Experimental**: this API might change without notice.
    pub fn ecdsa_msg(&mut self, idx: usize) -> Result<Sighash, SighashError> {
        let pset = self.pset;
        let input = input(pset, idx)?;
        if is_taproot_input(input) {
            return Err(SighashError::UnexpectedTaproot(idx));
        }
        spent_txout(input).ok_or(SighashError::MissingSpentOutput(idx))?;

        let msg = pset.sighash_msg(idx, &mut self.cache, None, BlockHash::all_zeros())?;
        match msg {
            PsbtSighashMsg::EcdsaSighash(sighash) => Ok(sighash),
            PsbtSighashMsg::TapSighash(_) => Err(SighashError::UnexpectedTaproot(idx)),
        }
    }

    /// The taproot message: key spend with no leaf, script spend with one.
    ///
    /// **Experimental**: this API might change without notice.
    pub fn taproot_msg(
        &mut self,
        idx: usize,
        leaf: Option<TapLeafHash>,
    ) -> Result<TapSighashHash, SighashError> {
        let pset = self.pset;
        let input = input(pset, idx)?;
        if !is_taproot_input(input) {
            return Err(SighashError::NotTaproot(idx));
        }
        let hash_ty = input
            .schnorr_hash_ty()
            .ok_or(SighashError::InvalidSchnorrSighashType(idx))?;
        let genesis_hash = self.genesis_hash.ok_or(SighashError::MissingGenesisHash)?;

        let mut prevouts = Vec::with_capacity(pset.inputs().len());
        for (i, input) in pset.inputs().iter().enumerate() {
            prevouts.push(spent_txout(input).ok_or(SighashError::MissingSpentOutput(i))?);
        }
        let prevouts = Prevouts::All(&prevouts);

        let msg = match leaf {
            Some(leaf) => self.cache.taproot_script_spend_signature_hash(
                idx,
                &prevouts,
                leaf,
                hash_ty,
                genesis_hash,
            )?,
            None => self.cache.taproot_key_spend_signature_hash(
                idx,
                &prevouts,
                hash_ty,
                genesis_hash,
            )?,
        };
        Ok(msg)
    }
}

fn input(pset: &PartiallySignedTransaction, idx: usize) -> Result<&Input, SighashError> {
    pset.inputs()
        .get(idx)
        .ok_or(SighashError::IndexOutOfBounds(idx))
}

#[cfg(test)]
mod tests {
    use elements::{
        confidential::Value, opcodes, pset::Input, script::Builder, sighash::SighashCache,
        EcdsaSighashType, OutPoint, Script, TxOut, Txid,
    };

    use super::*;

    #[test]
    fn ecdsa_msg_masks_the_pegin_outpoint_flag() {
        let witness_script = Builder::new()
            .push_opcode(opcodes::all::OP_PUSHNUM_1)
            .into_script();
        let value = Value::Explicit(1000);
        let txout = TxOut {
            script_pubkey: Script::new_v0_wsh(&witness_script.wscript_hash()),
            value,
            ..Default::default()
        };

        let mut input = Input::from_prevout(OutPoint::new(Txid::all_zeros(), 7 | PEGIN_FLAG));
        input.witness_script = Some(witness_script.clone());
        input.witness_utxo = Some(txout);

        let mut pset = PartiallySignedTransaction::new_v2();
        pset.add_input(input);

        let tx = pset.extract_tx().unwrap();
        assert!(tx.input[0].is_pegin());
        let mut masked = tx.clone();
        masked.input[0].previous_output.vout &= !PEGIN_FLAG;

        let unmasked_msg = SighashCache::new(&tx).segwitv0_sighash(
            0,
            &witness_script,
            value,
            EcdsaSighashType::All,
        );
        let masked_msg = SighashCache::new(&masked).segwitv0_sighash(
            0,
            &witness_script,
            value,
            EcdsaSighashType::All,
        );
        assert_ne!(unmasked_msg, masked_msg);

        let msg = SighashCtx::new(&pset, None).unwrap().ecdsa_msg(0).unwrap();
        assert_eq!(msg, masked_msg);
    }
}
