use elements::{
    bitcoin::{
        bip32::{DerivationPath, Fingerprint},
        PublicKey,
    },
    hashes::Hash,
    pset::{Input, PartiallySignedTransaction},
    secp256k1_zkp::{Message, XOnlyPublicKey},
    EcdsaSighashType, SchnorrSig, SchnorrSighashType, TxInWitness, TxOutWitness,
};
use elements_miniscript::psbt::SighashError;
use lwk_common::{get_genesis_hash, is_taproot_input, Network, SighashCtx};

use crate::{
    anti_exfil, derivation_path_to_vec, script_code_wpkh, sign_liquid_tx::TxInputParams, Error,
    SECP,
};

pub(crate) enum SignInfo {
    Ecdsa {
        public_key: PublicKey,
        host_entropy: [u8; 32],
        message: Message,
        sighash: u8,
    },
    Taproot {
        message: Message,
        output_key: XOnlyPublicKey,
        hash_ty: SchnorrSighashType,
    },
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

fn ecdsa_sighash(input: &Input) -> Result<EcdsaSighashType, SighashError> {
    // Per BIP 174, rust-elements defaults a missing sighash type to SIGHASH_ALL;
    // None therefore means an explicitly non-standard ECDSA sighash.
    input
        .ecdsa_hash_ty()
        .ok_or(SighashError::InvalidSighashType)
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
                SignInfo::Taproot { .. } => {
                    let schnorr_sig =
                        SchnorrSig::from_slice(&sig).map_err(|e| Error::Generic(e.to_string()))?;
                    input.tap_key_sig = Some(schnorr_sig);
                    *sigs_added_or_overwritten += 1;
                }
                SignInfo::Ecdsa { public_key, .. } => {
                    input.partial_sigs.insert(public_key, sig);
                    *sigs_added_or_overwritten += 1;
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn prepare_inputs(
    pset: &PartiallySignedTransaction,
    my_fingerprint: Fingerprint,
    network: Network,
) -> Result<Vec<(Option<SignInfo>, TxInputParams)>, Error> {
    let inputs = pset.inputs();

    let signing_taproot = inputs.iter().any(|input| {
        input
            .tap_key_origins
            .values()
            .any(|(_, (fingerprint, _))| fingerprint == &my_fingerprint)
    });

    let mut derivations = Vec::with_capacity(inputs.len());
    for (i, input) in inputs.iter().enumerate() {
        derivations.push(Derivation::from_input(
            input,
            my_fingerprint,
            i,
            is_taproot_input(input),
        )?);
    }

    let genesis_hash = get_genesis_hash(pset).unwrap_or(network.genesis_hash());
    let mut ctx = SighashCtx::new(pset, Some(genesis_hash))?;

    let mut prepared = Vec::with_capacity(inputs.len());
    for ((i, input), derivation) in inputs.iter().enumerate().zip(derivations) {
        prepared.push(prepare_input(
            input,
            i,
            derivation,
            signing_taproot,
            &mut ctx,
        )?);
    }

    Ok(prepared)
}

fn prepare_input(
    input: &Input,
    i: usize,
    derivation: Derivation<'_>,
    signing_taproot: bool,
    ctx: &mut SighashCtx,
) -> Result<(Option<SignInfo>, TxInputParams), Error> {
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
            let hash_ty = input
                .schnorr_hash_ty()
                .ok_or(SighashError::InvalidSighashType)?;

            let sighash = ctx.taproot_msg(i, None)?;

            // A P2TR scriptPubKey is OP_PUSHNUM_1, OP_PUSHBYTES_32, then the 32-byte output key.
            let output_key = XOnlyPublicKey::from_slice(&previous_output_script.as_bytes()[2..])
                .map_err(|_| Error::InvalidTaprootOutputKey(i))?;

            (
                Some(SignInfo::Taproot {
                    message: Message::from_digest(sighash.to_byte_array()),
                    output_key,
                    hash_ty,
                }),
                TxInputParams {
                    is_witness: Some(true),
                    script_code: previous_output_script.as_bytes().to_vec(),
                    value_commitment,
                    path: Some(derivation_path_to_vec(derivation_path)),
                    sighash: Some(hash_ty as u32),
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

            let sighash = ecdsa_sighash(input)?;
            let message = Message::from_digest(ctx.segwitv0_msg(i, &script_code)?.to_byte_array());
            let host_entropy = anti_exfil::new_host_entropy()?;
            let host_commitment = anti_exfil::host_commitment(&host_entropy);

            (
                Some(SignInfo::Ecdsa {
                    public_key: pk,
                    host_entropy,
                    message,
                    sighash: sighash as u8,
                }),
                TxInputParams {
                    is_witness: Some(true),
                    script_code: script_code.as_bytes().to_vec(),
                    // Jade's `value_commitment` input accepts the serialized Elements
                    // confidential::Value, including explicit 9-byte values. For more info:
                    // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/main/process/sign_tx.c#L612-L622
                    // (test case) https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/test_data/liquid_txn_nonconfidential_input.json#L54
                    value_commitment,
                    path: Some(derivation_path_to_vec(derivation_path)),
                    sighash: Some(sighash.as_u32()),
                    ae_host_commitment: host_commitment.to_vec(),
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

/// Prepare PSET for signature verifaction after the `sign_psbt`.
///
/// We are doing this because `sign_psbt` not only appends signatures, but also makes these changes:
/// - if the `genesis_hash` as in the ELIP-101 specification is not present, it sets it in the PSET, and
///   returns a version with the `genesis_hash` included;
///   (source: https://github.com/Blockstream/Jade/blob/9c097297f58339c15fb9b8df4c7fe105efefb902/main/process/sign_psbt.c#L735-L747)
/// - witnesses for the `txout` of `witness_utxo` and the `txin`/`txout` of `non_witness_utxo`
///   are being dropped during serialization, so they are cleared as well.
///   (source: https://github.com/ElementsProject/libwally-core/blob/3bf543cd06a67fdd877688a6304808f270351aee/src/psbt.c#L3107-L3114)
pub(crate) fn prepare_pset_sign_psbt(
    pset: &PartiallySignedTransaction,
    network: &Network,
) -> PartiallySignedTransaction {
    let mut pset = pset.clone();

    lwk_common::set_genesis_hash(&mut pset, network);

    for input in pset.inputs_mut() {
        if let Some(txout) = input.witness_utxo.as_mut() {
            txout.witness = TxOutWitness::empty();
        }
        if let Some(tx) = input.non_witness_utxo.as_mut() {
            for txin in tx.input.iter_mut() {
                txin.witness = TxInWitness::empty();
            }
            for txout in tx.output.iter_mut() {
                txout.witness = TxOutWitness::empty();
            }
        }
    }
    pset
}

pub(crate) fn validate_signature(
    sign_info: &Option<SignInfo>,
    signer_commitment: &[u8],
    signature: &[u8],
) -> Result<(), Error> {
    match sign_info {
        Some(SignInfo::Ecdsa {
            public_key,
            host_entropy,
            message,
            sighash,
        }) => anti_exfil::verify_der(
            &public_key.inner,
            message,
            host_entropy,
            signer_commitment,
            signature,
            *sighash,
        )
        .map_err(|_| Error::SignatureValidationFailed),
        Some(SignInfo::Taproot {
            message,
            output_key,
            hash_ty,
        }) => {
            // AE covers ECDSA only
            let signature =
                SchnorrSig::from_slice(signature).map_err(|_| Error::SignatureValidationFailed)?;
            if signature.hash_ty != *hash_ty {
                return Err(Error::SignatureValidationFailed);
            }
            SECP.verify_schnorr(&signature.sig, message, output_key)
                .map_err(|_| Error::SignatureValidationFailed)
        }
        None if signature.is_empty() => Ok(()),
        None => Err(Error::SignatureValidationFailed),
    }
}

#[cfg(test)]
mod tests {
    use elements::{
        pset::PsbtSighashType,
        secp256k1_zkp::{Keypair, Secp256k1, XOnlyPublicKey},
        EcdsaSighashType, SchnorrSig, SchnorrSighashType,
    };
    use elements_miniscript::psbt::SighashError;

    use super::{ecdsa_sighash, validate_signature, Error, Input, Message, SignInfo};

    #[test]
    fn ecdsa_sighash_defaults_and_validates() {
        let mut input = Input::default();
        assert_eq!(ecdsa_sighash(&input).unwrap(), EcdsaSighashType::All);

        input.sighash_type = Some(PsbtSighashType::from_u32(
            EcdsaSighashType::SinglePlusAnyoneCanPay.as_u32(),
        ));
        assert_eq!(
            ecdsa_sighash(&input).unwrap(),
            EcdsaSighashType::SinglePlusAnyoneCanPay
        );

        input.sighash_type = Some(PsbtSighashType::from_u32(0));
        assert!(matches!(
            ecdsa_sighash(&input),
            Err(SighashError::InvalidSighashType)
        ));
    }

    #[test]
    fn validate_taproot_signature() {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_seckey_slice(&secp, &[0x11; 32]).unwrap();
        let (output_key, _) = XOnlyPublicKey::from_keypair(&keypair);
        let message = Message::from_digest([0x22u8; 32]);
        let sig = secp.sign_schnorr_no_aux_rand(&message, &keypair);

        let sign_info = |hash_ty| {
            Some(SignInfo::Taproot {
                message,
                output_key,
                hash_ty,
            })
        };
        let signature = |hash_ty| SchnorrSig { sig, hash_ty }.to_vec();

        let default_sig = signature(SchnorrSighashType::Default);
        let all_sig = signature(SchnorrSighashType::All);

        validate_signature(&sign_info(SchnorrSighashType::Default), &[], &default_sig).unwrap();
        validate_signature(&sign_info(SchnorrSighashType::All), &[], &all_sig).unwrap();

        let mut corrupted = default_sig.clone();
        corrupted[10] ^= 0x01;
        assert!(matches!(
            validate_signature(&sign_info(SchnorrSighashType::Default), &[], &corrupted),
            Err(Error::SignatureValidationFailed)
        ));

        // the signature is valid, but it does not commit to the sighash we asked for
        assert!(matches!(
            validate_signature(&sign_info(SchnorrSighashType::All), &[], &default_sig),
            Err(Error::SignatureValidationFailed)
        ));
        assert!(matches!(
            validate_signature(&sign_info(SchnorrSighashType::Default), &[], &all_sig),
            Err(Error::SignatureValidationFailed)
        ));
    }
}
