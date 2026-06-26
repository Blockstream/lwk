use elements::{
    bitcoin::{
        bip32::{DerivationPath, Fingerprint},
        PublicKey,
    },
    pset::{Input, PartiallySignedTransaction},
    secp256k1_zkp::schnorr::Signature as SchnorrSignature,
    SchnorrSig, SchnorrSighashType,
};

use crate::{derivation_path_to_vec, script_code_wpkh, sign_liquid_tx::TxInputParams, Error};

pub(crate) enum SignInfo {
    Ecdsa(PublicKey),
    Taproot,
}

enum Derivation<'a> {
    Taproot(Option<&'a DerivationPath>),
    Ecdsa(Option<(&'a PublicKey, &'a DerivationPath)>),
}

impl<'a> Derivation<'a> {
    fn from_input(
        input: &'a Input,
        my_fingerprint: Fingerprint,
        i: usize,
        is_taproot: bool,
    ) -> Result<Self, Error> {
        if is_taproot {
            let mut tap_derivations = input
                .tap_key_origins
                .iter()
                .filter(|(_, (_, (fingerprint, _)))| fingerprint == &my_fingerprint);
            let tap_derivation = tap_derivations.next();

            if tap_derivations.next().is_some() {
                return Err(Error::MultipleTapDerivationsInput(i));
            }

            Ok(Self::Taproot(
                tap_derivation.map(|(_, (_, (_, derivation_path)))| derivation_path),
            ))
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

            Ok(Self::Ecdsa(jade_derivation.map(
                |(want_public_key, (_, derivation_path))| (want_public_key, derivation_path),
            )))
        }
    }
}

pub(crate) fn apply_sig(
    pset: &mut PartiallySignedTransaction,
    sign_info: Option<SignInfo>,
    sig: Vec<u8>,
    i: usize,
    sigs_added_or_overwritten: &mut u32,
) -> Result<(), Error> {
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
                    *sigs_added_or_overwritten += 1;
                }
                SignInfo::Ecdsa(public_key) => {
                    input.partial_sigs.insert(public_key, sig);
                    *sigs_added_or_overwritten += 1;
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn prepare_input(
    input: &Input,
    my_fingerprint: Fingerprint,
    i: usize,
    signing_taproot: bool,
) -> Result<(Option<SignInfo>, TxInputParams), Error> {
    let is_taproot = input
        .witness_utxo
        .as_ref()
        .map(|txout| txout.script_pubkey.is_v1_p2tr())
        .unwrap_or(false);

    let derivation = Derivation::from_input(input, my_fingerprint, i, is_taproot)?;

    let is_signable = matches!(
        derivation,
        Derivation::Taproot(Some(_)) | Derivation::Ecdsa(Some(_))
    );

    let txout = input.witness_utxo.as_ref();

    if (signing_taproot || is_signable) && txout.is_none() {
        return Err(Error::MissingWitnessUtxoInInput(i));
    }

    // If any input to be signed is taproot (p2tr), then 'scriptpubkey', 'value_commitment' and 'asset_generator'
    // are required for all inputs in the transaction.
    let (scriptpubkey, asset_generator, value_commitment) = if let Some(txout) = txout {
        (
            txout.script_pubkey.as_bytes().to_vec(),
            elements::encode::serialize(&txout.asset),
            elements::encode::serialize(&txout.value),
        )
    } else {
        Default::default()
    };

    let (sign_info, params) = match derivation {
        Derivation::Taproot(Some(derivation_path)) => {
            let previous_output_script =
                &txout.expect("is_signable => txout present").script_pubkey;
            (
                Some(SignInfo::Taproot),
                TxInputParams {
                    is_witness: Some(true),
                    script_code: previous_output_script.as_bytes().to_vec(),
                    value_commitment,
                    path: Some(derivation_path_to_vec(derivation_path)),
                    sighash: Some(0), // SIGHASH_DEFAULT per BIP341
                    // Must be empty for taproot: AE is not supported for P2TR inputs
                    ae_host_commitment: vec![],
                    // `scriptpubkey` is stored in the scriptpubkeys map used for the
                    // BIP341 sighash (distinct from the `script` field above)
                    scriptpubkey,
                    asset_generator,
                },
            )
        }
        Derivation::Ecdsa(Some((&pk, derivation_path))) => {
            let previous_output_script =
                &txout.expect("is_signable => txout present").script_pubkey;
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

            (
                Some(SignInfo::Ecdsa(pk)),
                TxInputParams {
                    is_witness: Some(true),
                    script_code: script_code.as_bytes().to_vec(),
                    // Jade's `value_commitment` input accepts the serialized Elements
                    // confidential::Value, including explicit 9-byte values. For more info:
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L612-L622
                    // (test case) https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/test_data/liquid_txn_nonconfidential_input.json#L54
                    value_commitment,
                    path: Some(derivation_path_to_vec(derivation_path)),
                    sighash: Some(1),
                    ae_host_commitment: vec![1u8; 32], // TODO verify anti-exfil
                    scriptpubkey,
                    asset_generator,
                },
            )
        }
        Derivation::Taproot(None) | Derivation::Ecdsa(None) =>
        // Jade expects one `tx_input` for every transaction input. Omitting
        // `path` marks this input as not signed by Jade; the matching reply
        // stays empty, and AE flow still exchanges `get_signature` per input.
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-request-anti-exfil
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-input-reply-anti-exfil (see bullets section)
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L570-L578
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L322-L372
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/jadepy/jade.py#L1837-L1840
        {
            (
                None,
                TxInputParams {
                    scriptpubkey,
                    asset_generator,
                    value_commitment,
                    ..Default::default()
                },
            )
        }
    };

    Ok((sign_info, params))
}
