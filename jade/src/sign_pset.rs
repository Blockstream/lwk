use elements::{encode::serialize, pset::PartiallySignedTransaction, Script};

use crate::{
    derivation_path_to_vec,
    protocol::GetSignatureParams,
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

    #[error("Missing witness utxo in input {0}")]
    MissingWitnessUtxoInInput(usize),

    #[error("Non confidential input {0}")]
    NonConfidentialInput(usize),

    #[error("Expecting bip 32 derivation for input {0}")]
    MissingBip32DerivInput(usize),
}

impl Jade {
    pub fn sign_pset(&mut self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        let tx = pset.extract_tx()?;
        let txn = serialize(&tx);
        let mut sigs_added_or_overwritten = 0;

        let mut trusted_commitments = vec![];
        let mut change = vec![];
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
            change.push(None); //TODO
        }

        let params = SignLiquidTxParams {
            network: crate::Network::LocaltestLiquid,
            txn,
            num_inputs: tx.input.len() as u32,
            use_ae_signatures: true,
            change,
            asset_info: vec![], // TODO
            trusted_commitments,
            additional_info: None,
        };
        let sign_response = self.sign_liquid_tx(params)?.get();
        assert!(sign_response);

        for (i, input) in pset.inputs_mut().iter_mut().enumerate() {
            let mut iter = input.bip32_derivation.clone().into_iter();
            let entry = iter.next().ok_or(Error::MissingBip32DerivInput(i))?;
            if iter.next().is_some() {
                panic!("other bip32 derivations..."); // TODO
            }
            let path: Vec<u32> = derivation_path_to_vec(&entry.1 .1);
            // TODO multisig

            let txout = input
                .witness_utxo
                .as_ref()
                .ok_or(Error::MissingWitnessUtxoInInput(i))?;

            let previous_output_script = &txout.script_pubkey;

            let params = TxInputParams {
                is_witness: true,
                script_code: script_code_wpkh(previous_output_script).as_bytes().to_vec(),
                value_commitment: txout
                    .value
                    .commitment()
                    .ok_or(Error::NonConfidentialInput(i))?
                    .serialize()
                    .to_vec(),
                path,
                sighash: Some(1),
                ae_host_commitment: vec![1u8; 32],
            };
            let _signer_commitment: Vec<u8> = self.tx_input(params)?.into();
        }

        for (i, input) in pset.inputs_mut().iter_mut().enumerate() {
            let mut iter = input.bip32_derivation.clone().into_iter();
            let entry = iter.next().ok_or(Error::MissingBip32DerivInput(i))?;

            let params = GetSignatureParams {
                ae_host_entropy: vec![1u8; 32],
            };
            let sig: Vec<u8> = self.get_signature_for_tx(params)?.into();

            input.partial_sigs.insert(entry.0, sig);
            sigs_added_or_overwritten += 1;
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
