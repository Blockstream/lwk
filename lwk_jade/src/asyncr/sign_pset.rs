use std::collections::HashMap;

use elements::pset::PartiallySignedTransaction;

use crate::{
    create_jade_sign_req, derivation_path_to_vec, protocol::GetSignatureParams, script_code_wpkh,
    sign_liquid_tx::TxInputParams, Error,
};

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

        let mut signers_commitment = HashMap::new();

        for (i, input) in pset.inputs_mut().iter_mut().enumerate() {
            for (want_public_key, (fingerprint, derivation_path)) in input.bip32_derivation.iter() {
                if &my_fingerprint == fingerprint {
                    let path: Vec<u32> = derivation_path_to_vec(derivation_path);

                    // TODO? verify `want_public_key` is one of the key of the descriptor?

                    let txout = input
                        .witness_utxo
                        .as_ref()
                        .ok_or(Error::MissingWitnessUtxoInInput(i))?;

                    let previous_output_script = &txout.script_pubkey;

                    let is_nested_wpkh = previous_output_script.is_p2sh()
                        && input
                            .redeem_script
                            .as_ref()
                            .map(|x| x.is_v0_p2wpkh())
                            .unwrap_or(false);

                    let script_code = if previous_output_script.is_v0_p2wpkh() {
                        script_code_wpkh(previous_output_script)
                    } else if previous_output_script.is_v0_p2wsh() {
                        input
                            .witness_script
                            .clone()
                            .ok_or(Error::MissingWitnessScript(i))?
                    } else if is_nested_wpkh {
                        script_code_wpkh(
                            input
                                .redeem_script
                                .as_ref()
                                .expect("Redeem script non-empty checked earlier"),
                        )
                    } else {
                        return Err(Error::UnsupportedScriptPubkeyType(
                            previous_output_script.asm(),
                        ));
                    };

                    let params = TxInputParams {
                        is_witness: true,
                        script_code: script_code.as_bytes().to_vec(),
                        value_commitment: txout
                            .value
                            .commitment()
                            .ok_or(Error::NonConfidentialInput(i))?
                            .serialize()
                            .to_vec(),
                        path,
                        sighash: Some(1),
                        ae_host_commitment: vec![1u8; 32], // TODO verify anti-exfil
                    };
                    let signer_commitment: Vec<u8> = self.tx_input(params).await?.to_vec();
                    signers_commitment.insert(*want_public_key, signer_commitment);
                }
            }
        }

        for input in pset.inputs_mut().iter_mut() {
            for (public_key, (_, _)) in input.bip32_derivation.iter() {
                if let Some(_signer_commitment) = signers_commitment.get(public_key) {
                    let params = GetSignatureParams {
                        ae_host_entropy: vec![1u8; 32], // TODO verify anti-exfil
                    };
                    let sig: Vec<u8> = self.get_signature_for_tx(params).await?.to_vec();

                    input.partial_sigs.insert(*public_key, sig);
                    sigs_added_or_overwritten += 1;
                }
            }
        }

        Ok(sigs_added_or_overwritten)
    }
}
