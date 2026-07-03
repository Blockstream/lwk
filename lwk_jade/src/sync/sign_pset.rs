use elements::pset::PartiallySignedTransaction;

use crate::{
    create_jade_sign_req,
    protocol::GetSignatureParams,
    sign_pset_common::{apply_sig, prepare_input, SignInfo},
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

        let has_taproot = pset.inputs().iter().any(|input| {
            input
                .tap_key_origins
                .values()
                .any(|(_, (fingerprint, _))| fingerprint == &my_fingerprint)
        });

        let mut sigs_added_or_overwritten = 0;
        let sign_response = self.sign_liquid_tx(params)?;
        assert!(sign_response);

        let mut signable_inputs: Vec<(Option<SignInfo>, Vec<u8>)> =
            Vec::with_capacity(pset.inputs().len());

        for (i, input) in pset.inputs().iter().enumerate() {
            let (sign_info, params) = prepare_input(input, my_fingerprint, i, has_taproot)?;
            let signer_commitment = self.tx_input(params)?.to_vec();
            signable_inputs.push((sign_info, signer_commitment));
        }

        for (i, (sign_info, _signer_commitment)) in signable_inputs.into_iter().enumerate() {
            // Jade rejects a non-empty `ae_host_commitment` for taproot inputs outright, so
            // prepare_input always sends an empty commitment for them:
            // https://github.com/Blockstream/Jade/blob/18fdfd074b143b00a1217736b9358de748fa7730/main/process/process_utils.c#L385
            let ae_host_entropy = match &sign_info {
                Some(SignInfo::Taproot) => vec![],
                _ => vec![1u8; 32], // TODO verify anti-exfil
            };
            let params = GetSignatureParams { ae_host_entropy };
            let sig: Vec<u8> = self.get_signature_for_tx(params)?.to_vec();

            apply_sig(pset, sign_info, sig, i, &mut sigs_added_or_overwritten)?;
        }

        Ok(sigs_added_or_overwritten)
    }
}
