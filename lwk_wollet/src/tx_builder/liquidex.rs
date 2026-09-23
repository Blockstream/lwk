//! Finishing a LiquiDEX proposal (maker) or a transaction taking one (taker)

use std::{
    collections::{BTreeMap, HashMap},
    vec,
};

use super::BuiltTx;
use super::TxBuilder;
use crate::{
    liquidex::{self, LiquidexError},
    Error, Wollet, EC,
};
use elements::{
    confidential::{AssetBlindingFactor, Nonce, ValueBlindingFactor},
    pset::{Output, PartiallySignedTransaction, PsbtSighashType},
    secp256k1_zkp, AssetId, EcdsaSighashType, RangeProof, Script, SurjectionProof,
};
use lwk_common::calculate_fee;
use rand::thread_rng;

impl TxBuilder {
    /// Finish building a transaction that can be converted to a LiquiDEX proposal
    pub(super) fn finish_liquidex_make(self, wollet: &Wollet) -> Result<BuiltTx, Error> {
        // Create PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();
        let mut inp_weight = 0;

        // Get input outpoint
        let selected_utxos = self
            .selected_utxos
            .ok_or(LiquidexError::MakerInvalidParams)?;
        let &[outpoint] = selected_utxos.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::MakerInvalidParams));
        };
        // Get output recipient
        let [recipient] = self.recipients.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::MakerInvalidParams));
        };

        // Add input
        let utxos = wollet.utxos_map()?;
        let utxo = utxos
            .get(&outpoint)
            .ok_or(Error::MissingWalletUtxo(outpoint))?;
        wollet.add_input(
            &mut pset,
            &mut inp_txout_sec,
            &mut inp_weight,
            utxo,
            self.add_input_rangeproofs,
        )?;

        let input = &mut pset.inputs_mut()[0];
        // Set asset blinding factor
        let txoutsecrets = inp_txout_sec.get(&0).expect("just added");
        input.set_abf(txoutsecrets.asset_bf);
        // Set blind value proof
        let input_scalar_offset = liquidex::scalar_offset(txoutsecrets);
        let blind_value_proof = liquidex::blind_value_proof(txoutsecrets)?;
        input.blind_value_proof = Some(blind_value_proof);

        // Set sighash
        input.sighash_type = Some(PsbtSighashType::from_u32(
            EcdsaSighashType::SinglePlusAnyoneCanPay.as_u32(),
        ));

        // Add output
        wollet.add_output(&mut pset, recipient)?;

        // Blind
        let asset = recipient.asset;
        let value = recipient.satoshi;
        let receiver_blinding_pk = recipient
            .blinding_pubkey
            .ok_or(LiquidexError::MakerInvalidParams)?;
        let script_pubkey = &recipient.script_pubkey;
        let mut rng = rand::thread_rng();
        let abf = AssetBlindingFactor::new(&mut rng);
        let vbf = ValueBlindingFactor::new(&mut rng);
        let (nonce, shared_secret) = Nonce::new_confidential(&mut rng, &EC, &receiver_blinding_pk);
        let ecdh_pubkey =
            elements::bitcoin::PublicKey::new(nonce.commitment().expect("confidential"));
        let asset_tag = secp256k1_zkp::Tag::from(asset.to_byte_array());
        let asset_generator =
            secp256k1_zkp::Generator::new_blinded(&EC, asset_tag, abf.into_inner());
        let value_commitment =
            secp256k1_zkp::PedersenCommitment::new(&EC, value, vbf.into_inner(), asset_generator);
        let min_value = if script_pubkey.is_provably_unspendable() {
            0
        } else {
            1
        };

        fn make_rangeproof_message(asset: AssetId, bf: secp256k1_zkp::Tweak) -> [u8; 64] {
            let mut message = [0u8; 64];

            message[..32].copy_from_slice(&asset.to_byte_array());
            message[32..].copy_from_slice(bf.as_ref());

            message
        }

        let message = make_rangeproof_message(asset, abf.into_inner());

        let rangeproof = RangeProof::new(
            &EC,
            min_value,
            value_commitment,
            value,
            vbf.into_inner(),
            &message,
            script_pubkey.as_bytes(),
            shared_secret,
            0,
            52,
            asset_generator,
        )?;

        let output = &mut pset.outputs_mut()[0];
        output.asset_comm = Some(asset_generator);
        output.amount_comm = Some(value_commitment);
        output.ecdh_pubkey = Some(ecdh_pubkey);
        output.value_rangeproof = Some(rangeproof);
        // We need to set an asset surjection proof, otherwise rust-elements does not serialize the PSET
        // https://github.com/ElementsProject/rust-elements/blob/master/src/pset/map/output.rs#L581
        let bytes = [
            1, 0, 1, 69, 162, 31, 81, 9, 102, 83, 180, 22, 237, 171, 76, 161, 122, 220, 124, 208,
            90, 74, 148, 162, 247, 161, 89, 3, 139, 112, 101, 185, 126, 78, 3, 188, 6, 32, 154,
            164, 175, 175, 158, 239, 225, 188, 83, 222, 42, 159, 10, 155, 216, 114, 78, 89, 163,
            124, 134, 74, 83, 104, 116, 254, 137, 218, 19,
        ];
        let fake_surjectionproof = SurjectionProof::from_slice(&bytes).expect("hardcoded");
        output.asset_surjection_proof = Some(fake_surjectionproof);
        output.set_abf(abf);
        let txoutsecrets = elements::TxOutSecrets {
            asset,
            asset_bf: abf,
            value,
            value_bf: vbf,
        };
        let output_scalar_offset = liquidex::scalar_offset(&txoutsecrets);
        let blind_value_proof = liquidex::blind_value_proof(&txoutsecrets)?;
        output.blind_value_proof = Some(blind_value_proof);
        // Add blind asset proof
        // Note: this is technically redundant since one could use the asset and abf,
        // but we add it to make things easier for verifiers,
        // so they don't have to handle the liquidex special case.
        let blind_asset_proof = SurjectionProof::blind_asset_proof(&mut rng, &EC, asset, abf)?;
        output.blind_asset_proof = Some(blind_asset_proof);

        // Add scalar
        // Compute the scalar offset to be added to the last vbf by the Taker to balance the transaction:
        // abf_i * value_i + vbf_i - (abf_o * value_o + vbf_o)
        let mut tweak = ValueBlindingFactor::from_slice(input_scalar_offset.as_ref())?;
        tweak += -ValueBlindingFactor::from_slice(output_scalar_offset.as_ref())?;
        pset.global.scalars = vec![tweak.into_inner()];

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        // TODO: blinding nonces
        Ok(BuiltTx {
            pset,
            blind_secrets: BTreeMap::new(),
        })
    }

    /// Finish building a transaction that takes LiquiDEX proposals
    pub(super) fn finish_liquidex_take(self, wollet: &Wollet) -> Result<BuiltTx, Error> {
        let [proposal] = self.liquidex_proposals.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };

        // Create PSET
        let mut pset = proposal.to_pset()?;
        let mut inp_txout_sec = HashMap::new();
        let mut inp_weight = 0;
        let mut input_domain = vec![];
        let mut last_unused_internal = wollet.change(None)?.index();
        let mut last_unused_external = wollet.address(None)?.index();
        let mut rng = thread_rng();

        let utxos = wollet.utxos_map()?;

        let [input] = pset.inputs() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };
        let maker_input_asset = input.asset.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_input_satoshi = input.amount.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_input_abf = input.get_abf().ok_or(LiquidexError::TakerInvalidParams)??;
        let [output] = pset.outputs() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };
        let maker_output_asset = output.asset.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_output_satoshi = output.amount.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_output_abf = output
            .get_abf()
            .ok_or(LiquidexError::TakerInvalidParams)??;

        // Maker input
        let surj_input = elements::SurjectionInput::Known {
            asset: maker_input_asset,
            asset_bf: maker_input_abf,
        };
        input_domain.push(surj_input.surjection_target(&EC).expect("known"));
        // In general the maker input is the only input with its asset, thus so need to pass its
        // asset and abf to "blind_last" so that it can create the surjection proof for the outputs
        // with its asset.
        // However the only way to pass these data to "blind_last" is through a TxOutSecrets in the
        // inp_txout_sec map, but we don't have the maker input value blinding factor (vbf).
        // Therefore we choose a vbf that has a *zero* scalar offset, so that is does not affect the
        // last vbf computation and rangeproof creation (its contribution to the last vbf is
        // already in the scalars field). Nevertheless when creating the surjection proofs for
        // outputs which asset is the same as the maker input, it can access the asset and abf from
        // inp_txout_sec.
        // vbf = - abf * v;
        let value_bf =
            ValueBlindingFactor::last(&EC, maker_input_satoshi, maker_input_abf, &[], &[]);
        let maker_input_txout_sec = elements::TxOutSecrets {
            asset: maker_input_asset,
            asset_bf: maker_input_abf,
            value: maker_input_satoshi,
            value_bf,
        };
        let idx = 0;
        inp_txout_sec.insert(idx, maker_input_txout_sec);

        // Add taker output (from proposal)
        let addressee = wollet.addressee_external(
            maker_input_satoshi,
            maker_input_asset,
            &mut last_unused_external,
        )?;
        wollet.add_output(&mut pset, &addressee)?;

        // Add inputs and change for maker output (if not L-BTC)
        // FIXME: If the wallet is taking a proposal made by the wallet itself, do not add the "maker" input again.
        if maker_output_asset != wollet.policy_asset() {
            let satoshi_out = maker_output_satoshi;
            let mut satoshi_in = 0;
            for utxo in utxos
                .values()
                .filter(|u| u.unblinded.asset == maker_output_asset)
            {
                wollet.add_input(
                    &mut pset,
                    &mut inp_txout_sec,
                    &mut inp_weight,
                    utxo,
                    self.add_input_rangeproofs,
                )?;
                let surj_input = elements::SurjectionInput::from_txout_secrets(utxo.unblinded);
                input_domain.push(surj_input.surjection_target(&EC).expect("from secrets"));
                satoshi_in += utxo.unblinded.value;
                if satoshi_in >= satoshi_out {
                    if satoshi_in > satoshi_out {
                        let satoshi_change = satoshi_in - satoshi_out;
                        let addressee = wollet.addressee_change(
                            satoshi_change,
                            maker_output_asset,
                            &mut last_unused_internal,
                        )?;
                        wollet.add_output(&mut pset, &addressee)?;
                    }
                    break;
                }
            }
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds {
                    missing_sats: satoshi_out - satoshi_in,
                    asset_id: maker_output_asset,
                });
            }
        }

        // Add inputs, change and fees for L-BTC
        let mut satoshi_out = 0;
        let mut satoshi_in = 0;
        if maker_output_asset == wollet.policy_asset() {
            satoshi_out += maker_output_satoshi;
        }
        if maker_input_asset == wollet.policy_asset() {
            satoshi_in += maker_input_satoshi;
            // We added the taker output above
            satoshi_out += maker_input_satoshi;
        }

        // FIXME: For implementation simplicity now we always add all L-BTC inputs
        for utxo in utxos
            .values()
            .filter(|u| u.unblinded.asset == wollet.policy_asset())
        {
            wollet.add_input(
                &mut pset,
                &mut inp_txout_sec,
                &mut inp_weight,
                utxo,
                self.add_input_rangeproofs,
            )?;
            let surj_input = elements::SurjectionInput::from_txout_secrets(utxo.unblinded);
            input_domain.push(surj_input.surjection_target(&EC).expect("from secrets"));
            satoshi_in += utxo.unblinded.value;
        }

        // Add a temporary fee, and always add a change or drain output,
        // then we'll tweak those values to match the given fee rate.
        let temp_fee = 1;
        if satoshi_in <= (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: (satoshi_out + temp_fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - temp_fee;
        let addressee = wollet.addressee_change(
            satoshi_change,
            wollet.policy_asset(),
            &mut last_unused_internal,
        )?;
        wollet.add_output(&mut pset, &addressee)?;
        let fee_output =
            Output::new_explicit(Script::default(), temp_fee, wollet.policy_asset(), None);
        pset.add_output(fee_output);

        for (vout, output) in pset.outputs_mut().iter_mut().enumerate() {
            // For the maker output, create the surjection proof
            if vout == 0 {
                let surjectionproof = SurjectionProof::new(
                    &EC,
                    &mut rng,
                    maker_output_asset,
                    maker_output_abf,
                    &input_domain,
                )?;

                output.asset_surjection_proof = Some(surjectionproof);
                output.blinder_index = None;
            }
            // Set all blinder index to 1 except for the maker output (1st) and the fee
            if (vout > 0) && !output.script_pubkey.is_empty() {
                output.blinder_index = Some(1);
            }
        }

        let weight = {
            let mut temp_pset = pset.clone();
            temp_pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            let tx_weight = {
                let tx = temp_pset.extract_tx()?;
                if self.ct_discount {
                    tx.discount_weight()
                } else {
                    tx.weight()
                }
            };
            inp_weight + tx_weight
        };

        let fee = calculate_fee(weight, self.fee_rate.to_sat_kvb());
        if satoshi_in <= satoshi_out.saturating_add(fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: satoshi_out.saturating_add(fee).saturating_add(1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - fee;
        // Replace change and fee outputs
        let n_outputs = pset.n_outputs();
        let outputs = pset.outputs_mut();
        let change_output = &mut outputs[n_outputs - 2]; // index check: we always have the lbtc change and the fee output at least
        change_output.amount = Some(satoshi_change);
        let fee_output = &mut outputs[n_outputs - 1];
        fee_output.amount = Some(fee);

        // Blind the transaction
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        // TODO: blinding nonces
        Ok(BuiltTx {
            pset,
            blind_secrets: BTreeMap::new(),
        })
    }
}
