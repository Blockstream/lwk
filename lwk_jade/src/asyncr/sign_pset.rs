use elements::{bitcoin::PublicKey, pset::PartiallySignedTransaction};

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

        let mut signable_inputs: Vec<(Option<PublicKey>, Vec<u8>)> =
            Vec::with_capacity(pset.inputs().len());

        for (i, input) in pset.inputs().iter().enumerate() {
            let mut jade_derivations = input
                .bip32_derivation
                .iter()
                .filter(|(_, (fingerprint, _))| &my_fingerprint == fingerprint);
            let jade_derivation = jade_derivations.next();
            if jade_derivations.next().is_some() {
                // Jade signs at most one path per tx_input message. Failing here is
                // safer than silently signing the first matching key and leaving the
                // remaining Jade-owned keys unsigned.
                return Err(Error::MultipleBip32DerivationsInput(i));
            }

            // TODO: handle `tap_key_origins` for taproot case
            if jade_derivation.is_none()
                && input
                    .tap_key_origins
                    .values()
                    .any(|(_, (fingerprint, _))| fingerprint == &my_fingerprint)
            {
                let desc = input
                    .witness_utxo
                    .as_ref()
                    .map(|u| u.script_pubkey.asm())
                    .unwrap_or_else(|| "taproot".to_string());
                return Err(Error::UnsupportedScriptPubkeyType(desc));
            }

            if let Some((want_public_key, (_, derivation_path))) = jade_derivation {
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
                    is_witness: Some(true),
                    script_code: script_code.as_bytes().to_vec(),
                    // Jade's `value_commitment` input accepts the serialized Elements
                    // confidential::Value, including explicit 9-byte values. For more info:
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L612-L622
                    // (test case) https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/test_data/liquid_txn_nonconfidential_input.json#L54
                    value_commitment: elements::encode::serialize(&txout.value),
                    path: Some(path),
                    sighash: Some(1),
                    ae_host_commitment: vec![1u8; 32], // TODO verify anti-exfil
                };
                let signer_commitment = self.tx_input(params).await?.to_vec();
                signable_inputs.push((Some(*want_public_key), signer_commitment));
            } else {
                // Jade expects one `tx_input` for every transaction input. Omitting
                // `path` marks this input as not signed by Jade; the matching reply
                // stays empty, and AE flow still exchanges `get_signature` per input.
                // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-request-anti-exfil
                // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-reply-anti-exfil (see bullets section)
                // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L570-L578
                // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L322-L372
                // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/jadepy/jade.py#L1837-L1840
                let signer_commitment = self.tx_input(TxInputParams::default()).await?.to_vec();
                signable_inputs.push((None, signer_commitment));
            }
        }

        for (i, (public_key, _signer_commitment)) in signable_inputs.into_iter().enumerate() {
            let params = GetSignatureParams {
                ae_host_entropy: vec![1u8; 32], // TODO verify anti-exfil
            };
            let sig: Vec<u8> = self.get_signature_for_tx(params).await?.to_vec();

            if let Some(public_key) = public_key {
                if !sig.is_empty() {
                    let input = pset.inputs_mut().get_mut(i).ok_or(Error::Generic(
                        "expected signable_inputs to have same length as pset.inputs()".to_string(),
                    ))?;

                    input.partial_sigs.insert(public_key, sig);
                    sigs_added_or_overwritten += 1;
                }
            }
        }

        Ok(sigs_added_or_overwritten)
    }
}
