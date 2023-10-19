use elements::{
    bitcoin::bip32::ChildNumber, encode::serialize, pset::PartiallySignedTransaction, Script,
};
use std::collections::HashMap;

use crate::{
    derivation_path_to_vec,
    get_receive_address::{SingleOrMulti, Variant},
    protocol::GetSignatureParams,
    sign_liquid_tx::{Change, Commitment, SignLiquidTxParams, TxInputParams},
    Jade,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Pset(#[from] elements::pset::Error),

    #[error(transparent)]
    Jade(#[from] crate::Error),

    #[error("Missing asset id in output {0}")]
    MissingAssetIdInOutput(usize),

    #[error("Missing blind asset proof in output {0}")]
    MissingBlindAssetProofInOutput(usize),

    #[error("Missing asset commitment in output {0}")]
    MissingAssetCommInOutput(usize),

    #[error("Missing blinding key in output {0}")]
    MissingBlindingKeyInOutput(usize),

    #[error("Missing amount in output {0}")]
    MissingAmountInOutput(usize),

    #[error("Missing amount commitment in output {0}")]
    MissingAmountCommInOutput(usize),

    #[error("Missing blind value proof in output {0}")]
    MissingBlindValueProofInOutput(usize),

    #[error("Missing witness utxo in input {0}")]
    MissingWitnessUtxoInInput(usize),

    #[error("Non confidential input {0}")]
    NonConfidentialInput(usize),

    #[error("Expecting bip 32 derivation for input {0}")]
    MissingBip32DerivInput(usize),

    #[error("Previous script pubkey is wsh but witness script is missing in input {0}")]
    MissingWitnessScript(usize),

    #[error("Unsupported spending script pubkey: {0}")]
    UnsupportedScriptPubkeyType(String),
}

const CHANGE_CHAIN: ChildNumber = ChildNumber::Normal { index: 1 };

impl Jade {
    pub fn sign_pset(&mut self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        let tx = pset.extract_tx()?;
        let txn = serialize(&tx);
        let mut sigs_added_or_overwritten = 0;
        let my_fingerprint = self.fingerprint()?;

        let mut trusted_commitments = vec![];
        let mut changes = vec![];
        for (i, output) in pset.outputs().iter().enumerate() {
            let mut asset_id = serialize(&output.asset.ok_or(Error::MissingAssetIdInOutput(i))?);
            asset_id.reverse(); // Jade want it reversed
            let burn_script = Script::new_op_return(&[]);
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
                    let is_change = path.clone().into_iter().nth_back(1) == Some(&CHANGE_CHAIN);
                    if is_change {
                        if output.script_pubkey.is_v0_p2wpkh() {
                            change = Some(Change {
                                address: SingleOrMulti::Single {
                                    variant: Variant::Wpkh,
                                    path: derivation_path_to_vec(path),
                                },
                                is_change: true,
                            });
                        } else if output.script_pubkey.is_p2sh() {
                            if let Some(redeem_script) = output.redeem_script.as_ref() {
                                if redeem_script.is_v0_p2wpkh() {
                                    change = Some(Change {
                                        address: SingleOrMulti::Single {
                                            variant: Variant::ShWpkh,
                                            path: derivation_path_to_vec(path),
                                        },
                                        is_change: true,
                                    });
                                }
                            }
                        }
                    }

                    // TODO handle multisig
                }
            }
            changes.push(change);
        }

        let params = SignLiquidTxParams {
            network: crate::Network::LocaltestLiquid,
            txn,
            num_inputs: tx.input.len() as u32,
            use_ae_signatures: true,
            change: changes,
            asset_info: vec![], // TODO
            trusted_commitments,
            additional_info: None,
        };
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
