use elements::{
    bitcoin::PublicKey, pset::PartiallySignedTransaction,
    secp256k1_zkp::schnorr::Signature as SchnorrSignature, SchnorrSig, SchnorrSighashType,
};

use crate::{
    create_jade_sign_req, derivation_path_to_vec, protocol::GetSignatureParams, script_code_wpkh,
    sign_liquid_tx::TxInputParams, Error, Jade,
};

enum SignInfo {
    Ecdsa(PublicKey),
    Taproot,
}

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
            let txout = input
                .witness_utxo
                .as_ref()
                .ok_or(Error::MissingWitnessUtxoInInput(i))?;
            let previous_output_script = &txout.script_pubkey;

            // When any taproot input is being signed, Jade needs the scriptpubkey and
            // asset generator for every input to build sha_scriptpubkeys / sha_assets.
            let tap_scriptpubkey = if has_taproot {
                previous_output_script.as_bytes().to_vec()
            } else {
                vec![]
            };
            let tap_asset_generator = if has_taproot {
                txout
                    .asset
                    .commitment()
                    .map(|g| g.serialize().to_vec())
                    .unwrap_or_default()
            } else {
                vec![]
            };

            if previous_output_script.is_v1_p2tr() {
                let mut tap_derivations = input
                    .tap_key_origins
                    .iter()
                    .filter(|(_, (_, (fingerprint, _)))| fingerprint == &my_fingerprint);
                let tap_derivation = tap_derivations.next();
                if tap_derivations.next().is_some() {
                    return Err(Error::MultipleTapDerivationsInput(i));
                }

                if let Some((_, (_, (_, derivation_path)))) = tap_derivation {
                    let path = derivation_path_to_vec(derivation_path);
                    let params = TxInputParams {
                        is_witness: Some(true),
                        // `script` (= script_code) is read by Jade to detect the P2TR type
                        script_code: previous_output_script.as_bytes().to_vec(),
                        value_commitment: elements::encode::serialize(&txout.value),
                        path: Some(path),
                        sighash: Some(0), // SIGHASH_DEFAULT per BIP341
                        // Must be empty for taproot: AE is not supported for P2TR inputs
                        ae_host_commitment: vec![],
                        // `scriptpubkey` is stored in the scriptpubkeys map used for the
                        // BIP341 sighash (distinct from the `script` field above)
                        scriptpubkey: tap_scriptpubkey,
                        asset_generator: tap_asset_generator,
                    };
                    let signer_commitment = self.tx_input(params)?.to_vec();
                    signable_inputs.push((Some(SignInfo::Taproot), signer_commitment));
                } else {
                    let params = TxInputParams {
                        scriptpubkey: tap_scriptpubkey,
                        asset_generator: tap_asset_generator,
                        value_commitment: elements::encode::serialize(&txout.value),
                        ..Default::default()
                    };
                    let signer_commitment = self.tx_input(params)?.to_vec();
                    signable_inputs.push((None, signer_commitment));
                }
            } else {
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

                if let Some((want_public_key, (_, derivation_path))) = jade_derivation {
                    let path: Vec<u32> = derivation_path_to_vec(derivation_path);

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
                        scriptpubkey: tap_scriptpubkey,
                        asset_generator: tap_asset_generator,
                    };
                    let signer_commitment = self.tx_input(params)?.to_vec();
                    signable_inputs
                        .push((Some(SignInfo::Ecdsa(*want_public_key)), signer_commitment));
                } else {
                    // Jade expects one `tx_input` for every transaction input. Omitting
                    // `path` marks this input as not signed by Jade; the matching reply
                    // stays empty, and AE flow still exchanges `get_signature` per input.
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-request-anti-exfil
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-reply-anti-exfil (see bullets section)
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L570-L578
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L322-L372
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/jadepy/jade.py#L1837-L1840
                    let params = TxInputParams {
                        scriptpubkey: tap_scriptpubkey,
                        asset_generator: tap_asset_generator,
                        value_commitment: elements::encode::serialize(&txout.value),
                        ..Default::default()
                    };
                    let signer_commitment = self.tx_input(params)?.to_vec();
                    signable_inputs.push((None, signer_commitment));
                }
            }
        }

        for (i, (sign_info, _signer_commitment)) in signable_inputs.into_iter().enumerate() {
            // Taproot Schnorr signatures have no AE support; commitment was sent empty, so
            // entropy must also be empty or Jade rejects with "commitment and entropy" mismatch.
            let ae_host_entropy = match &sign_info {
                Some(SignInfo::Taproot) => vec![],
                _ => vec![1u8; 32], // TODO verify anti-exfil
            };
            let params = GetSignatureParams { ae_host_entropy };
            let sig: Vec<u8> = self.get_signature_for_tx(params)?.to_vec();

            if let Some(sign_info) = sign_info {
                if !sig.is_empty() {
                    let input = pset.inputs_mut().get_mut(i).ok_or(Error::Generic(
                        "expected signable_inputs to have same length as pset.inputs()".to_string(),
                    ))?;

                    match sign_info {
                        SignInfo::Taproot => {
                            let schnorr_sig = SchnorrSignature::from_slice(&sig)
                                .map_err(|e| Error::Generic(e.to_string()))?;
                            input.tap_key_sig = Some(SchnorrSig {
                                sig: schnorr_sig,
                                hash_ty: SchnorrSighashType::Default,
                            });
                            sigs_added_or_overwritten += 1;
                        }
                        SignInfo::Ecdsa(public_key) => {
                            input.partial_sigs.insert(public_key, sig);
                            sigs_added_or_overwritten += 1;
                        }
                    }
                }
            }
        }

        Ok(sigs_added_or_overwritten)
    }
}
