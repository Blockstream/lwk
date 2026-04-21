use elements::pset::PartiallySignedTransaction;

use crate::{create_jade_sign_req, protocol::GetSignatureParams, tx_input_params, Error};

use super::{Jade, Stream};

impl<S: Stream<Error = Error>> Jade<S> {
    /// Sign a pset from a Jade
    pub async fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        let my_fingerprint = self.fingerprint().await?;

        // Singlesig signing don't need this, however, it is simpler to always ask for it and once cached is a
        // fast operation anyway (and in a real scenario you may ask for registered multisigs at the beginning of the session)
        let multisigs_details = self.get_cached_registered_multisigs().await?;
        let network = self.network;

        let params = create_jade_sign_req(pset, my_fingerprint, multisigs_details, network)?;

        let mut sigs_added_or_overwritten = 0;
        let sign_response = self.sign_liquid_tx(params).await?;
        assert!(sign_response);

        let mut signing_public_keys = Vec::with_capacity(pset.inputs().len());
        for (i, input) in pset.inputs().iter().enumerate() {
            let (params, signing_public_key) = tx_input_params(input, i, &my_fingerprint)?;
            let _signer_commitment: Vec<u8> = self.tx_input(params).await?.to_vec();
            signing_public_keys.push(signing_public_key);
        }

        for (input, signing_public_key) in pset.inputs_mut().iter_mut().zip(signing_public_keys) {
            let params = GetSignatureParams {
                ae_host_entropy: vec![1u8; 32], // TODO verify anti-exfil
            };
            let sig: Vec<u8> = self.get_signature_for_tx(params).await?.to_vec();
            if let Some(public_key) = signing_public_key {
                if !sig.is_empty() {
                    input.partial_sigs.insert(public_key, sig);
                    sigs_added_or_overwritten += 1;
                }
            }
        }

        Ok(sigs_added_or_overwritten)
    }
}
