use std::collections::HashMap;

use elements_miniscript::{
    elements::{
        confidential::{Asset, Value},
        pset::PartiallySignedTransaction,
        AssetId, OutPoint, TxOutSecrets,
    },
    ConfidentialDescriptor, DescriptorPublicKey,
};

use crate::wallet::derive_script_pubkey;

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
}

pub fn pset_balance(
    pset: &PartiallySignedTransaction,
    unblinded: &HashMap<OutPoint, TxOutSecrets>,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<PsetBalance, Error> {
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
                                    // FIXME consider longer than 1 derivation path and fingerprint if available
                                    let mine =
                                        derive_script_pubkey(descriptor, path[0].into()).unwrap();
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
        match (output.amount, output.asset) {
            (None, None) => return Err(Error::OutputAssetValueNone { output_index }),
            (None, Some(_)) => return Err(Error::OutputValueNone { output_index }),
            (Some(_), None) => return Err(Error::OutputAssetNone { output_index }),
            (Some(amount), Some(asset_id)) => {
                if output.script_pubkey.is_empty() {
                    fee = Some(amount);
                }
                for (_, path) in output.bip32_derivation.values() {
                    // FIXME consider longer than 1 derivation path and fingerprint if available
                    let mine = derive_script_pubkey(descriptor, path[0].into()).unwrap();
                    if mine == output.script_pubkey {
                        *balances.entry(asset_id).or_default() += amount as i64;
                        continue 'outputsfor;
                    }
                }
            }
        }
    }
    let fee = fee.ok_or(Error::MissingFee)?;

    Ok(PsetBalance { fee, balances })
}
