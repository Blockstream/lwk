use elements::pset::PartiallySignedTransaction;

use crate::{
    create_jade_sign_req,
    protocol::{GetSignatureParams, Request},
    sign_pset_common::{apply_sig, prepare_inputs, validate_signature, SignInfo},
    Error,
};

use super::Jade;

impl Jade {
    /// Sign a pset from a Jade
    pub fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        let my_fingerprint = self.fingerprint()?;

        // Singlesig signing don't need this, however, it is simpler to always ask for it and once cached is a
        // fast operation anyway (and in a real scenario you may ask for registered multisigs at the beginning of the session)
        let multisigs_details = self.get_cached_registered_multisigs()?;
        let network = self.network;

        let params = create_jade_sign_req(pset, my_fingerprint, multisigs_details, network)?;

        let prepared = prepare_inputs(pset, my_fingerprint, network)?;

        let mut sigs_added_or_overwritten = 0;

        // Keep one lock for the whole signing session so no other request can interleave
        // between `sign_liquid_tx`, `tx_input` and `get_signature`.
        let mut conn = self.conn.lock()?;
        let sign_response: bool = self.send_with_conn(Request::SignLiquidTx(params), &mut conn)?;
        if !sign_response {
            return Err(Error::SignTxRejected);
        }

        let mut signable_inputs: Vec<(Option<SignInfo>, Vec<u8>)> =
            Vec::with_capacity(prepared.len());

        for (sign_info, params) in prepared {
            let signer_commitment: serde_bytes::ByteBuf =
                self.send_with_conn(Request::TxInput(params), &mut conn)?;
            let signer_commitment = signer_commitment.to_vec();
            signable_inputs.push((sign_info, signer_commitment));
        }

        let mut signatures = Vec::with_capacity(signable_inputs.len());
        for (sign_info, signer_commitment) in signable_inputs {
            // Jade rejects a non-empty `ae_host_commitment` for taproot inputs outright, so
            // prepare_input always sends an empty commitment for them:
            // https://github.com/Blockstream/Jade/blob/18fdfd074b143b00a1217736b9358de748fa7730/main/process/process_utils.c#L385
            let ae_host_entropy = match &sign_info {
                Some(SignInfo::Taproot { .. }) => vec![],
                Some(SignInfo::Ecdsa { host_entropy, .. }) => host_entropy.to_vec(),
                None => vec![],
            };
            let params = GetSignatureParams { ae_host_entropy };
            let sig: serde_bytes::ByteBuf =
                self.send_with_conn(Request::GetSignature(params), &mut conn)?;
            let sig = sig.to_vec();
            validate_signature(&sign_info, &signer_commitment, &sig)?;
            signatures.push((sign_info, sig));
        }

        for (i, (sign_info, sig)) in signatures.into_iter().enumerate() {
            apply_sig(pset, sign_info, sig, i, &mut sigs_added_or_overwritten)?;
        }

        Ok(sigs_added_or_overwritten)
    }
}
