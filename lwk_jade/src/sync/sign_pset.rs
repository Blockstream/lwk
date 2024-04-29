use elements::{
    bitcoin::bip32::{ChildNumber, Fingerprint},
    encode::serialize,
    hex::ToHex,
    opcodes::{
        all::{OP_CHECKMULTISIG, OP_PUSHNUM_1, OP_PUSHNUM_16},
        All,
    },
    pset::PartiallySignedTransaction,
    script::Instruction,
    Script,
};
use std::collections::{HashMap, HashSet};

use crate::{
    derivation_path_to_vec,
    get_receive_address::{SingleOrMulti, Variant},
    protocol::GetSignatureParams,
    register_multisig::RegisteredMultisigDetails,
    sign_liquid_tx::{
        AssetInfo, Change, Commitment, Contract, Prevout, SignLiquidTxParams, TxInputParams,
    },
    Error, Jade, Network,
};
use lwk_common::burn_script;

const CHANGE_CHAIN: ChildNumber = ChildNumber::Normal { index: 1 };

impl Jade {
    /// Sign a pset from a Jade
    pub fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        let my_fingerprint = self.fingerprint()?;
        let multisigs_details = self.get_cached_registered_multisigs()?;
        let network = self.network;

        let params = create_jade_sign_req(pset, my_fingerprint, multisigs_details, network)?;

        let mut sigs_added_or_overwritten = 0;
        let sign_response = self.sign_liquid_tx(params)?;
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
                    let signer_commitment: Vec<u8> = self.tx_input(params)?.to_vec();
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
                    let sig: Vec<u8> = self.get_signature_for_tx(params)?.to_vec();

                    input.partial_sigs.insert(*public_key, sig);
                    sigs_added_or_overwritten += 1;
                }
            }
        }

        Ok(sigs_added_or_overwritten)
    }
}

fn create_jade_sign_req(
    pset: &mut PartiallySignedTransaction,
    my_fingerprint: Fingerprint,
    multisigs_details: Vec<RegisteredMultisigDetails>,
    network: Network,
) -> Result<SignLiquidTxParams, Error> {
    let tx = pset.extract_tx()?;
    let txn = serialize(&tx);
    let burn_script = burn_script();
    let mut asset_ids_in_tx = HashSet::new();
    let mut trusted_commitments = vec![];
    let mut changes = vec![];
    for (i, output) in pset.outputs().iter().enumerate() {
        let asset_id = output.asset.ok_or(Error::MissingAssetIdInOutput(i))?;
        asset_ids_in_tx.insert(asset_id);
        let mut asset_id = serialize(&asset_id);
        asset_id.reverse(); // Jade want it reversed
        let unblinded = output.script_pubkey.is_empty() || output.script_pubkey == burn_script;
        let trusted_commitment = if unblinded {
            // fee output or burn output
            None
        } else {
            Some(Commitment {
                asset_blind_proof: output
                    .blind_asset_proof
                    .as_ref()
                    .ok_or(Error::MissingBlindAssetProofInOutput(i))?
                    .serialize(),
                asset_generator: output
                    .asset_comm
                    .ok_or(Error::MissingAssetCommInOutput(i))?
                    .serialize()
                    .to_vec(),
                asset_id,
                blinding_key: output
                    .blinding_key
                    .ok_or(Error::MissingBlindingKeyInOutput(i))?
                    .to_bytes(),
                value: output.amount.ok_or(Error::MissingAmountInOutput(i))?,
                value_commitment: output
                    .amount_comm
                    .ok_or(Error::MissingAmountCommInOutput(i))?
                    .serialize()
                    .to_vec(),
                value_blind_proof: output
                    .blind_value_proof
                    .as_ref()
                    .ok_or(Error::MissingBlindValueProofInOutput(i))?
                    .serialize(),
            })
        };
        trusted_commitments.push(trusted_commitment);

        let mut change = None;
        for (fingerprint, path) in output.bip32_derivation.values() {
            if fingerprint == &my_fingerprint {
                // This ensures that path has at least 2 elements
                let is_change = path.clone().into_iter().nth_back(1) == Some(&CHANGE_CHAIN);
                if is_change {
                    if output.script_pubkey.is_v0_p2wpkh() {
                        change = Some(Change {
                            address: SingleOrMulti::Single {
                                variant: Variant::Wpkh,
                                path: derivation_path_to_vec(path),
                            },
                            is_change,
                        });
                    } else if output.script_pubkey.is_p2sh() {
                        if let Some(redeem_script) = output.redeem_script.as_ref() {
                            if redeem_script.is_v0_p2wpkh() {
                                change = Some(Change {
                                    address: SingleOrMulti::Single {
                                        variant: Variant::ShWpkh,
                                        path: derivation_path_to_vec(path),
                                    },
                                    is_change,
                                });
                            }
                        }
                    } else if output.script_pubkey.is_v0_p2wsh() {
                        if let Some(witness_script) = output.witness_script.as_ref() {
                            if is_multisig(witness_script) {
                                for details in &multisigs_details {
                                    // path has at least 2 elements
                                    let index = path[path.len() - 1];
                                    if let Ok(derived_witness_script) = details
                                        .descriptor
                                        .derive_witness_script(is_change, index.into())
                                    {
                                        if witness_script == &derived_witness_script {
                                            let mut paths = vec![];
                                            for _ in 0..details.descriptor.signers.len() {
                                                // FIXME: here we should only pass the paths that were
                                                // not passed when calling register_multisig. However
                                                // deducing them now is not trivial, thus we only take
                                                // the last 2 elements in the derivation path which we
                                                // expect to be "0|1,*"
                                                let v = derivation_path_to_vec(path);
                                                // path has at least 2 elements
                                                let v = v[(path.len() - 2)..].to_vec();
                                                paths.push(v);
                                            }
                                            change = Some(Change {
                                                address: SingleOrMulti::Multi {
                                                    multisig_name: details
                                                        .multisig_name
                                                        .to_string(),
                                                    paths,
                                                },
                                                is_change,
                                            });
                                            break; // No need to check for more multisigs
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        changes.push(change);
    }
    let mut assets_info = vec![];
    for asset_id in asset_ids_in_tx {
        if let Some(Ok(meta)) = pset.get_asset_metadata(asset_id) {
            if let Ok(contract) = serde_json::from_str::<Contract>(meta.contract()) {
                let asset_info = AssetInfo {
                    asset_id: asset_id.to_string(),
                    contract,
                    issuance_prevout: Prevout {
                        txid: meta.issuance_prevout().txid.to_hex(),
                        vout: meta.issuance_prevout().vout,
                    },
                };

                assets_info.push(asset_info);
            }
        }
    }
    let params = SignLiquidTxParams {
        network,
        txn,
        num_inputs: tx.input.len() as u32,
        use_ae_signatures: true,
        change: changes,
        asset_info: assets_info,
        trusted_commitments,
        additional_info: None,
    };
    Ok(params)
}

// Get a script from witness script pubkey hash
fn script_code_wpkh(script: &Script) -> Script {
    assert!(script.is_v0_p2wpkh());
    // ugly segwit stuff
    let mut script_code = vec![0x76u8, 0xa9, 0x14];
    script_code.extend(&script.as_bytes()[2..]);
    script_code.push(0x88);
    script_code.push(0xac);
    Script::from(script_code)
}

// taken and adapted from:
// https://github.com/rust-bitcoin/rust-bitcoin/blob/37daf4620c71dc9332c3e08885cf9de696204bca/bitcoin/src/blockdata/script/borrowed.rs#L266
// TODO remove once it's released
#[allow(dead_code)]
fn is_multisig(script: &Script) -> bool {
    fn decode_pushnum(op: All) -> Option<u8> {
        let start: u8 = OP_PUSHNUM_1.into_u8();
        let end: u8 = OP_PUSHNUM_16.into_u8();
        if start < op.into_u8() && end >= op.into_u8() {
            Some(op.into_u8() - start + 1)
        } else {
            None
        }
    }

    let required_sigs;

    let mut instructions = script.instructions();
    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if let Some(pushnum) = decode_pushnum(op) {
            required_sigs = pushnum;
        } else {
            return false;
        }
    } else {
        return false;
    }

    let mut num_pubkeys: u8 = 0;
    while let Some(Ok(instruction)) = instructions.next() {
        match instruction {
            Instruction::PushBytes(_) => {
                num_pubkeys += 1;
            }
            Instruction::Op(op) => {
                if let Some(pushnum) = decode_pushnum(op) {
                    if pushnum != num_pubkeys {
                        return false;
                    }
                }
                break;
            }
        }
    }

    if required_sigs > num_pubkeys {
        return false;
    }

    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if op != OP_CHECKMULTISIG {
            return false;
        }
    } else {
        return false;
    }

    instructions.next().is_none()
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use elements::Script;

    #[test]
    fn test_is_multisig() {
        let multisig = Script::from_str("522102ebc62c20f1e09e169a88745f60f6dac878c92db5c7ed78c6703d2d0426a01f942102c2d59d677122bc292048833003fd5cb19d27d32896b1d0feec654c291f7ede9e52ae").unwrap();
        assert_eq!(multisig.asm(), "OP_PUSHNUM_2 OP_PUSHBYTES_33 02ebc62c20f1e09e169a88745f60f6dac878c92db5c7ed78c6703d2d0426a01f94 OP_PUSHBYTES_33 02c2d59d677122bc292048833003fd5cb19d27d32896b1d0feec654c291f7ede9e OP_PUSHNUM_2 OP_CHECKMULTISIG");
        assert!(super::is_multisig(&multisig));

        let not_multisig =
            Script::from_str("001414fe45f2c2a2b7c00d0940d694a3b6af6c9bf165").unwrap();
        assert_eq!(
            not_multisig.asm(),
            "OP_0 OP_PUSHBYTES_20 14fe45f2c2a2b7c00d0940d694a3b6af6c9bf165"
        );
        assert!(!super::is_multisig(&not_multisig));
    }
}
//
