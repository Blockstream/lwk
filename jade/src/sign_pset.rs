use elements::{
    bitcoin::bip32::ChildNumber, confidential::Value, encode::serialize,
    pset::PartiallySignedTransaction,
};

use crate::{
    sign_liquid_tx::{Commitment, SignLiquidTxParams, TxInputParams},
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
}

impl Jade {
    pub fn sign_pset(&mut self, pset: &mut PartiallySignedTransaction) -> Result<(), Error> {
        let tx = pset.extract_tx()?;
        let txn = serialize(&tx);

        let mut trusted_commitments = vec![];
        for (i, output) in pset.outputs().iter().enumerate() {
            let mut asset_id = serialize(&output.asset.ok_or(Error::MissingAssetIdInOutput(i))?);
            asset_id.reverse(); // Jade want it reversed
            let trusted_commitment = if output.script_pubkey.is_empty() {
                // fee output
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
        }

        let params = SignLiquidTxParams {
            network: crate::Network::LocaltestLiquid,
            txn,
            num_inputs: tx.input.len() as u32,
            use_ae_signatures: false,
            change: vec![None, None, None], // TODO
            asset_info: vec![],             // TODO
            trusted_commitments,
            additional_info: None,
        };
        let sign_response = self.sign_liquid_tx(params)?.get();
        assert!(sign_response);

        for input in pset.inputs_mut() {
            let entry = input.bip32_derivation.clone().into_iter().next().unwrap();
            let params = TxInputParams {
                is_witness: true,
                script: input
                    .witness_utxo
                    .as_ref()
                    .unwrap()
                    .script_pubkey
                    .clone()
                    .into_bytes(),
                value_commitment: match input.witness_utxo.as_ref().unwrap().value {
                    Value::Null => panic!("null unexpected"),
                    Value::Explicit(_) => panic!("explicit unexpected"),
                    Value::Confidential(comm) => comm.serialize().to_vec(),
                },
                path: entry
                    .1
                     .1
                    .into_iter()
                    .map(|e| match e {
                        ChildNumber::Normal { index } => *index,
                        ChildNumber::Hardened { index: _ } => panic!("unexpected hardened deriv"),
                    })
                    .collect(),
                sighash: None,
            };
            let signature: Vec<u8> = self.tx_input(params)?.into();
            input.partial_sigs.insert(entry.0, signature);
        }

        Ok(())
    }
}
