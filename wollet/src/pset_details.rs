use std::collections::HashMap;

use crate::elements::{
    confidential::{Asset, Value},
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{All, Generator, PedersenCommitment, Secp256k1},
    AssetId, BlindAssetProofs, BlindValueProofs, OutPoint, TxOutSecrets,
};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};

use crate::sync::derive_blinding_key;
use crate::wallet::{convert_blinding_key, derive_script_pubkey};

#[derive(Debug)]
pub struct PsetBalance {
    pub fee: u64,
    pub balances: HashMap<AssetId, i64>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("There is no unblinding information and Input #{input_index} is missing witness_utxo of outpoint {previous_outpoint}")]
    MissingPreviousOutput {
        input_index: usize,
        previous_outpoint: OutPoint,
    },

    #[error("There is no unblinding information and Input #{input_index} has non explicit asset {asset}")]
    InputAssetNotExplicit { input_index: usize, asset: Asset },

    #[error("There is no unblinding information and Input #{input_index} has non explicit value {value}")]
    InputValueNotExplicit { input_index: usize, value: Value },

    #[error("Output #{output_index} has none asset")]
    OutputAssetNone { output_index: usize },

    #[error("Output #{output_index} has none value")]
    OutputValueNone { output_index: usize },

    #[error("Output #{output_index} has none value and none asset")]
    OutputAssetValueNone { output_index: usize },

    #[error("PSET doesn't contain a fee output")]
    MissingFee,

    #[error("Multiple fee outputs")]
    MultipleFee,

    #[error("Fee output is blinded")]
    BlindedFee,

    #[error("Output #{output_index} has invalid asset blind proof")]
    InvalidAssetBlindProof { output_index: usize },

    #[error("Output #{output_index} has invalid value blind proof")]
    InvalidValueBlindProof { output_index: usize },

    #[error("Output #{output_index} is not blinded")]
    OutputNotBlinded { output_index: usize },

    #[error("Output #{output_index} belongs to the wallet but cannot be unblinded")]
    OutputMineNotUnblindable { output_index: usize },

    #[error("Output #{output_index} belongs to the wallet but its commitments do not match the unblinded values")]
    OutputCommitmentsMismatch { output_index: usize },
}

fn commitments(
    secp: &Secp256k1<All>,
    txout_secrets: &TxOutSecrets,
) -> (Generator, PedersenCommitment) {
    let asset_comm = Generator::new_blinded(
        secp,
        txout_secrets.asset.into_inner().to_byte_array().into(),
        txout_secrets.asset_bf.into_inner(),
    );
    let amount_comm = PedersenCommitment::new(
        secp,
        txout_secrets.value,
        txout_secrets.value_bf.into_inner(),
        asset_comm,
    );
    (asset_comm, amount_comm)
}

pub fn pset_balance(
    pset: &PartiallySignedTransaction,
    unblinded: &HashMap<OutPoint, TxOutSecrets>,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<PsetBalance, Error> {
    let secp = Secp256k1::new();
    let descriptor_blinding_key =
        convert_blinding_key(&descriptor.key).expect("No private blinding keys for bare variant");
    let mut balances: HashMap<AssetId, i64> = HashMap::new();
    let mut fee: Option<u64> = None;
    'inputsfor: for (input_index, input) in pset.inputs().iter().enumerate() {
        let previous_outpoint = OutPoint {
            txid: input.previous_txid,
            vout: input.previous_output_index,
        };

        match unblinded.get(&previous_outpoint) {
            Some(tx_out_secrets) => {
                // TODO CHECK, if they are in unblinded they are surely mine?
                *balances.entry(tx_out_secrets.asset).or_default() -= tx_out_secrets.value as i64;
            }
            None => {
                // Try to get asset id and value from previous output if they are explicit
                match input.witness_utxo.as_ref() {
                    Some(utxo) => match utxo.asset {
                        Asset::Null | Asset::Confidential(_) => {
                            return Err(Error::InputAssetNotExplicit {
                                input_index,
                                asset: utxo.asset,
                            })
                        }
                        Asset::Explicit(asset_id) => match utxo.value {
                            Value::Null | Value::Confidential(_) => {
                                return Err(Error::InputValueNotExplicit {
                                    input_index,
                                    value: utxo.value,
                                })
                            }
                            Value::Explicit(value) => {
                                for (_, path) in input.bip32_derivation.values() {
                                    // TODO should I check descriptor derivation path is compatible with given bip32_derivation?
                                    // TODO consider fingerprint if available
                                    if path.is_empty() {
                                        continue;
                                    }
                                    let wildcard_index = path[path.len() - 1];
                                    let mine =
                                        derive_script_pubkey(descriptor, wildcard_index.into())
                                            .unwrap();
                                    if mine == utxo.script_pubkey {
                                        *balances.entry(asset_id).or_default() -= value as i64;
                                        continue 'inputsfor;
                                    }
                                }
                            }
                        },
                    },
                    None => {
                        return Err(Error::MissingPreviousOutput {
                            input_index,
                            previous_outpoint,
                        })
                    }
                }
            }
        }
    }

    'outputsfor: for (output_index, output) in pset.outputs().iter().enumerate() {
        if output.script_pubkey.is_empty() {
            // Candidate fee output
            if fee.is_some() {
                return Err(Error::MultipleFee);
            }
            if output.asset.is_none()
                || output.asset_comm.is_some()
                || output.amount.is_none()
                || output.amount_comm.is_some()
            {
                return Err(Error::BlindedFee);
            }
            fee = Some(output.amount.unwrap());
            continue 'outputsfor;
        }

        // Expect all outputs to be blinded and with blind proofs
        match (
            output.asset,
            output.asset_comm,
            output.blind_asset_proof.as_ref(),
            output.amount,
            output.amount_comm,
            output.blind_value_proof.as_ref(),
        ) {
            (None, _, _, None, _, _) => return Err(Error::OutputAssetValueNone { output_index }),
            (None, _, _, Some(_), _, _) => return Err(Error::OutputValueNone { output_index }),
            (Some(_), _, _, None, _, _) => return Err(Error::OutputAssetNone { output_index }),
            (
                Some(asset),
                Some(asset_comm),
                Some(blind_asset_proof),
                Some(amount),
                Some(amount_comm),
                Some(blind_value_proof),
            ) => {
                if !blind_asset_proof.blind_asset_proof_verify(&secp, asset, asset_comm) {
                    return Err(Error::InvalidAssetBlindProof { output_index });
                }
                if !blind_value_proof.blind_value_proof_verify(
                    &secp,
                    amount,
                    asset_comm,
                    amount_comm,
                ) {
                    return Err(Error::InvalidValueBlindProof { output_index });
                }
                for (_, path) in output.bip32_derivation.values() {
                    if path.is_empty() {
                        continue;
                    }
                    let wildcard_index = path[path.len() - 1];
                    // TODO should I check descriptor derivation path is compatible with given bip32_derivation?
                    // TODO consider fingerprint if available
                    let mine = derive_script_pubkey(descriptor, wildcard_index.into()).unwrap();
                    if mine == output.script_pubkey {
                        // Check that we can later unblind the output
                        let private_blinding_key =
                            derive_blinding_key(&output.script_pubkey, &descriptor_blinding_key);
                        let txout_secrets = output
                            .to_txout()
                            .unblind(&secp, private_blinding_key)
                            .map_err(|_| Error::OutputMineNotUnblindable { output_index })?;
                        if (asset_comm, amount_comm) != commitments(&secp, &txout_secrets) {
                            return Err(Error::OutputCommitmentsMismatch { output_index });
                        }

                        *balances.entry(asset).or_default() += amount as i64;
                        continue 'outputsfor;
                    }
                }
            }
            _ => return Err(Error::OutputNotBlinded { output_index }),
        }
    }
    let fee = fee.ok_or(Error::MissingFee)?;

    Ok(PsetBalance { fee, balances })
}
