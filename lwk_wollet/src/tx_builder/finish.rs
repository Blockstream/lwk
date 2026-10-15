//! Finishing an ordinary transaction

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    vec,
};

use super::TxBuilder;
use super::{add_external_input, measure_weight};
use super::{extract_issuances, needs_blinding, BuiltTx};
use crate::{
    issuance::{IssuanceRequest, Issuances, ReissuanceRequest},
    model::{ExternalUtxo, Recipient},
    pset_create::SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS,
    Chain, Error, WalletTxOut, Wollet, EC,
};
use elements::{
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
    pset::{Output, PartiallySignedTransaction},
    OutPoint, Script, TxOutSecrets,
};
use elements_miniscript::psbt::PsbtExt;
use lwk_common::{calculate_fee, set_genesis_hash};
use rand::thread_rng;

impl TxBuilder {
    pub(super) fn finish_inner(self, wollet: &Wollet) -> Result<BuiltTx, Error> {
        if !self.allow_high_fee_rate && self.fee_rate.to_sat_kvb() > Self::MAX_FEE_RATE {
            return Err(Error::FeeRateTooHigh(self.fee_rate.to_sat_kvb()));
        }

        if !self.pegin_inputs.is_empty() {
            if self.inputs_order.is_some() {
                return Err(Error::PeginUnsupportedBuilderMode("manual inputs order"));
            }
            if self.is_liquidex_make || !self.liquidex_proposals.is_empty() {
                return Err(Error::PeginUnsupportedBuilderMode("LiquiDEX"));
            }
        }

        if self.is_liquidex_make {
            if self.inputs_order.is_some() {
                return Err(Error::LiquidexUnsupportedWithInputsOrder);
            }

            return self.finish_liquidex_make(wollet);
        } else if !self.liquidex_proposals.is_empty() {
            if self.inputs_order.is_some() {
                return Err(Error::LiquidexUnsupportedWithInputsOrder);
            }

            return self.finish_liquidex_take(wollet);
        }

        if let Some(selected_outpoints) = self.selected_utxos.as_ref() {
            let mut outpoints_set = HashSet::new();

            for &outpoint in selected_outpoints {
                if !outpoints_set.insert(outpoint) {
                    return Err(Error::DuplicatedOutpoint(
                        outpoint,
                        "manual coin selection".to_string(),
                    ));
                }
            }
        }

        if let Some(inputs_order) = self.inputs_order.as_ref() {
            let selected_outpoints = self
                .selected_utxos
                .as_ref()
                .ok_or(Error::InputsOrderRequiresWalletUtxos)?;

            let mut ordered = HashSet::new();
            for &outpoint in inputs_order {
                if !ordered.insert(outpoint) {
                    return Err(Error::DuplicatedOutpoint(
                        outpoint,
                        "manual inputs order".to_string(),
                    ));
                }
            }

            let mut expected: HashSet<OutPoint> = selected_outpoints.iter().copied().collect();
            expected.extend(self.external_utxos.iter().map(|utxo| utxo.outpoint));

            if ordered != expected {
                return Err(Error::InputsOrderUtxosMismatch);
            }

            // `inputs_order` forbids adding any input beyond them, so a pinned
            // reissuance input must be one of them.
            for request in &self.reissuances.requests {
                if let Some(outpoint) = request.pinned_input {
                    if !ordered.contains(&outpoint) {
                        return Err(Error::ReissuanceOutpointNotInInputsOrder(outpoint));
                    }
                }
            }
        }
        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();

        set_genesis_hash(&mut pset, &self.network());
        let mut inp_txout_sec = HashMap::new();
        let mut last_unused_internal = wollet.last_unused_internal();
        let mut last_unused_external = wollet.last_unused_external();

        let mut inp_weight = 0;

        let utxos = wollet.utxos_map()?;

        let policy_asset = *self.network().policy_asset();
        let (addressees_lbtc, addressees_asset): (Vec<_>, Vec<_>) = self
            .recipients
            .into_iter()
            .partition(|a| a.asset == policy_asset);

        // Get selected utxos (manual coin selection)
        let mut selected_utxos = vec![];
        if let Some(ref coins) = self.selected_utxos {
            for coin in coins {
                let utxo = utxos.get(coin).ok_or(Error::MissingWalletUtxo(*coin))?;
                selected_utxos.push(utxo);
            }
        }

        // Assets that belongs to this transaction
        // all the ones with a recipient
        let mut assets: HashSet<_> = addressees_asset.iter().map(|a| a.asset).collect();
        // and all the ones of utxos that are being added
        for utxo in &self.external_utxos {
            assets.insert(utxo.unblinded.asset);
        }
        for utxo in &selected_utxos {
            assets.insert(utxo.unblinded.asset);
        }
        // Policy asset is handled separately below
        assets.remove(&policy_asset);

        enum Item<'a> {
            Own(&'a WalletTxOut),
            External(&'a ExternalUtxo),
        }

        for asset in assets {
            let mut satoshi_out = 0;
            let mut satoshi_in = 0;

            for addressee in addressees_asset.iter().filter(|a| a.asset == asset) {
                wollet.add_output(&mut pset, addressee)?;
                satoshi_out += addressee.satoshi;
            }

            let mut gathered = Vec::new();

            for utxo in self
                .external_utxos
                .iter()
                .filter(|utxo| utxo.unblinded.asset == asset)
            {
                gathered.push(Item::External(utxo));
                satoshi_in += utxo.unblinded.value;
            }

            if self.selected_utxos.is_some() {
                // Add only selected asset utxos
                for &utxo in selected_utxos
                    .iter()
                    .filter(|utxo| utxo.unblinded.asset == asset)
                {
                    gathered.push(Item::Own(utxo));
                    satoshi_in += utxo.unblinded.value;
                }
            } else {
                let mut auto_utxos = utxos.values().filter(|u| u.unblinded.asset == asset);

                while satoshi_in < satoshi_out {
                    let Some(utxo) = auto_utxos.next() else {
                        break;
                    };

                    gathered.push(Item::Own(utxo));
                    satoshi_in += utxo.unblinded.value;
                }
            }

            // Add change
            if satoshi_in > satoshi_out {
                let satoshi_change = satoshi_in - satoshi_out;
                let addressee =
                    wollet.addressee_change(satoshi_change, asset, &mut last_unused_internal)?;
                wollet.add_output(&mut pset, &addressee)?;
            }

            // Insufficient funds
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds {
                    missing_sats: satoshi_out - satoshi_in,
                    asset_id: asset,
                });
            }

            if self.inputs_order.is_none() {
                for utxo in gathered {
                    match utxo {
                        Item::Own(utxo) => wollet.add_input(
                            &mut pset,
                            &mut inp_txout_sec,
                            &mut inp_weight,
                            utxo,
                            self.add_input_rangeproofs,
                        )?,
                        Item::External(utxo) => add_external_input(
                            &mut pset,
                            &mut inp_txout_sec,
                            &mut inp_weight,
                            utxo,
                            self.add_input_rangeproofs,
                        )?,
                    };
                }
            }
        }

        if let Some(inputs_order) = self.inputs_order.as_ref() {
            let external_by_outpoint = self
                .external_utxos
                .iter()
                .map(|eu| (eu.outpoint, eu))
                .collect::<HashMap<_, _>>();
            for outpoint in inputs_order {
                if let Some(utxo) = utxos.get(outpoint) {
                    wollet.add_input(
                        &mut pset,
                        &mut inp_txout_sec,
                        &mut inp_weight,
                        utxo,
                        self.add_input_rangeproofs,
                    )?;
                } else {
                    add_external_input(
                        &mut pset,
                        &mut inp_txout_sec,
                        &mut inp_weight,
                        external_by_outpoint
                            .get(outpoint)
                            .expect("set equality checked at the top"),
                        self.add_input_rangeproofs,
                    )?;
                }
            }
        }

        let issuances_len = match &self.issuances {
            Issuances::Sequential(requests) => requests.len(),
            Issuances::None | Issuances::Pinned(_) => 0,
        };

        let mut lbtc_candidates: Vec<&WalletTxOut> = vec![];

        // L-BTC inputs and outputs
        // Fee and L-BTC change after (re)issuance
        let mut satoshi_out = 0;
        let mut satoshi_in = 0;
        for addressee in addressees_lbtc {
            wollet.add_output(&mut pset, &addressee)?;
            satoshi_out += addressee.satoshi;
        }

        // Add pegins as explicit L-BTC inputs.
        for pegin_input in &self.pegin_inputs {
            if pset.inputs().len() >= SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS {
                return Err(Error::TooManyInputs(pset.inputs().len()));
            }

            let amount = pegin_input.funding().deposit().amount().to_sat();
            let input = pegin_input.to_pset_input()?;
            pset.add_input(input);
            let idx = pset.inputs().len() - 1;
            let derivation_index = pegin_input
                .funding()
                .deposit()
                .pegin_address()
                .derivation_index();
            let descriptor = wollet
                .wollet_descriptor()
                .definite_descriptor(Chain::External, derivation_index)?;
            pset.update_input_with_descriptor(idx, &descriptor)?;
            // An explicit input has zero asset and value blinding factors. Supplying
            // those factors lets `blind_last` balance confidential outputs against it.
            inp_txout_sec.insert(
                idx,
                TxOutSecrets::new(
                    policy_asset,
                    AssetBlindingFactor::zero(),
                    amount,
                    ValueBlindingFactor::zero(),
                ),
            );
            inp_weight += wollet.max_weight_to_satisfy();
            satoshi_in += amount;
        }

        // Add all external L-BTC utxos
        for utxo in &self.external_utxos {
            if utxo.unblinded.asset != policy_asset {
                continue;
            }
            if self.inputs_order.is_none() {
                add_external_input(
                    &mut pset,
                    &mut inp_txout_sec,
                    &mut inp_weight,
                    utxo,
                    self.add_input_rangeproofs,
                )?;
            }
            satoshi_in += utxo.unblinded.value;
        }

        if self.selected_utxos.is_some() {
            for utxo in &selected_utxos {
                if utxo.unblinded.asset != policy_asset {
                    continue;
                }
                if self.inputs_order.is_none() {
                    wollet.add_input(
                        &mut pset,
                        &mut inp_txout_sec,
                        &mut inp_weight,
                        utxo,
                        self.add_input_rangeproofs,
                    )?;
                }
                satoshi_in += utxo.unblinded.value;
            }
        } else {
            // Add L-BTC inputs to cover the recipients, largest utxos first
            lbtc_candidates = utxos
                .values()
                .filter(|u| u.unblinded.asset == policy_asset)
                .collect();

            lbtc_candidates.sort_by(|a, b| {
                a.unblinded
                    .value
                    .cmp(&b.unblinded.value)
                    .then(a.outpoint.cmp(&b.outpoint))
            });

            while self.drain_lbtc || satoshi_in < satoshi_out || pset.n_inputs() < issuances_len {
                let Some(utxo) = lbtc_candidates.pop() else {
                    break;
                };
                wollet.add_input(
                    &mut pset,
                    &mut inp_txout_sec,
                    &mut inp_weight,
                    utxo,
                    self.add_input_rangeproofs,
                )?;
                satoshi_in += utxo.unblinded.value;
            }
        }

        let mut inputs_idx_map: HashMap<OutPoint, usize> = pset
            .inputs()
            .iter()
            .enumerate()
            .map(|(idx, i)| (OutPoint::new(i.previous_txid, i.previous_output_index), idx))
            .collect();

        let mut set_issuance = |idx: usize,
                                request: IssuanceRequest,
                                pset: &mut PartiallySignedTransaction|
         -> Result<(), Error> {
            let (asset, token) = wollet.set_issuance(
                pset,
                idx,
                request.satoshi_asset,
                request.satoshi_token,
                request.contract,
            )?;

            for output in request.asset_outputs {
                let addressee = match output.address {
                    Some(address) => Recipient::from_address(output.satoshi, &address, asset),
                    None => wollet.addressee_external(
                        output.satoshi,
                        asset,
                        &mut last_unused_external,
                    )?,
                };
                wollet.add_output(pset, &addressee)?;
            }

            for output in request.token_outputs {
                let addressee = match output.address {
                    Some(address) => Recipient::from_address(output.satoshi, &address, token),
                    None => wollet.addressee_external(
                        output.satoshi,
                        token,
                        &mut last_unused_external,
                    )?,
                };
                wollet.add_output(pset, &addressee)?;
            }

            Ok(())
        };

        match self.issuances {
            Issuances::None => {}
            Issuances::Sequential(requests) => {
                if self.inputs_order.is_some() {
                    return Err(Error::InputsOrderRequiresPinnedIssuance);
                }

                if requests.len() > pset.n_inputs() {
                    return Err(Error::IssuanceInputCountMismatch);
                }

                for (idx, request) in requests.into_iter().enumerate() {
                    set_issuance(idx, request, &mut pset)?;
                }
            }
            Issuances::Pinned(requests) => {
                let Some(inputs_order) = self.inputs_order.as_ref() else {
                    return Err(Error::IssuancePinRequiresInputsOrder);
                };

                if inputs_order.len() < requests.len() {
                    return Err(Error::IssuanceInputCountMismatch);
                }

                let mut pinned_outpoints = HashSet::new();
                for (_, outpoint) in &requests {
                    if !pinned_outpoints.insert(*outpoint) {
                        return Err(Error::DuplicatedOutpoint(
                            *outpoint,
                            "pinned issuances".to_string(),
                        ));
                    }
                }

                for (request, outpoint) in requests {
                    let idx = inputs_idx_map
                        .get(&outpoint)
                        .ok_or(Error::IssuanceOutpointNotInInputsOrder(outpoint))?;

                    set_issuance(*idx, request, &mut pset)?;
                }
            }
        }

        let mut set_reissuance = |request: ReissuanceRequest,
                                  pset: &mut PartiallySignedTransaction|
         -> Result<(), Error> {
            let issuance = if let Some(issuance_tx) = request.issuance_tx {
                extract_issuances(&issuance_tx)
                    .iter()
                    .find(|i| i.asset == request.asset_to_reissue)
                    .ok_or_else(|| Error::MissingIssuance)?
                    .clone()
            } else {
                wollet.issuance(&request.asset_to_reissue)?
            };
            let token = issuance.token;

            enum TokenInput<'a> {
                InInputs(usize, AssetBlindingFactor),
                InWallet(&'a WalletTxOut),
            }

            // Looks for the (pinned) token among inputs, or in wallet utxos to add one.
            // A pinned outpoint is checked above to be among inputs_order, when set.
            let token_input = match request.pinned_input {
                Some(outpoint) => {
                    let (pinned, asset) = if let Some(inp_idx) = inputs_idx_map.get(&outpoint) {
                        let u = inp_txout_sec.get(inp_idx).ok_or(Error::Generic(format!(
                            "input {inp_idx} has no txout secrets"
                        )))?;
                        (TokenInput::InInputs(*inp_idx, u.asset_bf), u.asset)
                    } else if let Some(u) = utxos.get(&outpoint) {
                        (TokenInput::InWallet(u), u.unblinded.asset)
                    } else {
                        return Err(Error::MissingWalletUtxo(outpoint));
                    };

                    if asset != token {
                        return Err(Error::ReissuancePinnedInputNotToken { outpoint, token });
                    }
                    pinned
                }
                None => match inp_txout_sec
                    .iter()
                    .filter(|(_, u)| u.asset == token)
                    .min_by_key(|(idx, _)| **idx)
                {
                    Some((idx, u)) => TokenInput::InInputs(*idx, u.asset_bf),
                    None => {
                        if self.inputs_order.is_some() {
                            // The token utxo isn't among the pinned inputs, and
                            // `inputs_order` forbids adding any input beyond them.
                            return Err(Error::TokenUtxoNotInInputsOrder(token));
                        }
                        TokenInput::InWallet(
                            utxos
                                .values()
                                .filter(|u| u.unblinded.asset == token)
                                .min_by_key(|u| u.outpoint)
                                .ok_or_else(|| Error::MissingReissuanceTokenUtxo(token))?,
                        )
                    }
                },
            };

            let (idx, token_asset_bf) = match token_input {
                TokenInput::InInputs(idx, asset_bf) => (idx, asset_bf),
                TokenInput::InWallet(utxo_token) => {
                    // Add an input sending the token,
                    let idx = wollet.add_input(
                        pset,
                        &mut inp_txout_sec,
                        &mut inp_weight,
                        utxo_token,
                        self.add_input_rangeproofs,
                    )?;
                    inputs_idx_map.insert(utxo_token.outpoint, idx);

                    // and an outpout receiving the token
                    let satoshi_token = utxo_token.unblinded.value;
                    let addressee =
                        wollet.addressee_change(satoshi_token, token, &mut last_unused_internal)?;
                    wollet.add_output(pset, &addressee)?;

                    (idx, utxo_token.unblinded.asset_bf)
                }
            };

            // Set reissuance data
            wollet.set_reissuance(
                pset,
                idx,
                request.satoshi_to_reissue,
                &token_asset_bf,
                &issuance.entropy,
            )?;

            for output in request.asset_outputs {
                let addressee = match output.address {
                    Some(address) => {
                        Recipient::from_address(output.satoshi, &address, request.asset_to_reissue)
                    }
                    None => wollet.addressee_external(
                        output.satoshi,
                        request.asset_to_reissue,
                        &mut last_unused_external,
                    )?,
                };
                wollet.add_output(pset, &addressee)?;
            }

            Ok(())
        };

        // Set (re)issuance data
        for request in self.reissuances.requests {
            set_reissuance(request, &mut pset)?;
        }

        // TODO: consider a zero value output to balance the blinders

        let temp_fee = 1;
        while satoshi_in <= (satoshi_out + temp_fee) {
            let Some(utxo) = lbtc_candidates.pop() else {
                return Err(Error::InsufficientFunds {
                    missing_sats: (satoshi_out + temp_fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                    asset_id: wollet.policy_asset(),
                });
            };
            wollet.add_input(
                &mut pset,
                &mut inp_txout_sec,
                &mut inp_weight,
                utxo,
                self.add_input_rangeproofs,
            )?;
            satoshi_in += utxo.unblinded.value;
        }
        let satoshi_change = satoshi_in - satoshi_out - temp_fee;
        let addressee = if let Some(address) = self.drain_to {
            Recipient::from_address(satoshi_change, &address, wollet.policy_asset())
        } else {
            wollet.addressee_change(
                satoshi_change,
                wollet.policy_asset(),
                &mut last_unused_internal,
            )?
        };
        wollet.add_output(&mut pset, &addressee)?;
        let fee_output =
            Output::new_explicit(Script::default(), temp_fee, wollet.policy_asset(), None);
        pset.add_output(fee_output);

        // Measure the fee and, if it's not covered, add more
        // inputs and measure again.
        let mut fee = calculate_fee(
            measure_weight(&pset, &inp_txout_sec, inp_weight, self.ct_discount)?,
            self.fee_rate.to_sat_kvb(),
        );
        while satoshi_in <= satoshi_out.saturating_add(fee) {
            let missing_sats = satoshi_out.saturating_add(fee).saturating_add(1) - satoshi_in; // +1 to ensure we have more than just equal
            if lbtc_candidates.is_empty() {
                return Err(Error::InsufficientFunds {
                    missing_sats,
                    asset_id: wollet.policy_asset(),
                });
            }

            let mut missing = missing_sats;
            while missing > 0 {
                let Some(utxo) = lbtc_candidates.pop() else {
                    break;
                };
                wollet.add_input(
                    &mut pset,
                    &mut inp_txout_sec,
                    &mut inp_weight,
                    utxo,
                    self.add_input_rangeproofs,
                )?;
                satoshi_in += utxo.unblinded.value;
                missing = missing.saturating_sub(utxo.unblinded.value);
            }

            fee = calculate_fee(
                measure_weight(&pset, &inp_txout_sec, inp_weight, self.ct_discount)?,
                self.fee_rate.to_sat_kvb(),
            );
        }
        let satoshi_change = satoshi_in - satoshi_out - fee;
        // Replace change and fee outputs
        let n_outputs = pset.n_outputs();
        let outputs = pset.outputs_mut();
        let change_output = &mut outputs[n_outputs - 2]; // index check: we always have the lbtc change and the fee output at least
        change_output.amount = Some(satoshi_change);
        let fee_output = &mut outputs[n_outputs - 1];
        fee_output.amount = Some(fee);

        // TODO inputs/outputs(except fee) randomization, not trivial because of blinder_index on inputs

        // Blind the transaction
        let mut built_tx = if needs_blinding(&pset)? {
            let mut rng = thread_rng();

            let blind_secrets = pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            // Remove the output witness (surjection and range proofs) from the utxos of the inputs.
            // PSET (elements/doc/pset.mediawiki, PSBT_ELEMENTS_IN_UTXO_RANGEPROOF) does not
            // serialize the witness data of the UTXOs, so the PSET must not hold it in memory
            // either, otherwise it differs from its serialized form (e.g. from the PSET returned
            // by a cosigner).
            // For non witness utxos, this also appears to be necessary for pre-segwit inputs.
            for input in pset.inputs_mut() {
                if let Some(ref mut tx) = &mut input.non_witness_utxo {
                    for output in &mut tx.output {
                        output.witness = Default::default();
                    }
                }
                if let Some(ref mut txout) = &mut input.witness_utxo {
                    txout.witness = Default::default();
                }
            }
            BuiltTx {
                pset,
                blind_secrets,
            }
        } else {
            BuiltTx {
                pset,
                blind_secrets: BTreeMap::new(),
            }
        };

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut built_tx.pset)?;

        Ok(built_tx)
    }
}
