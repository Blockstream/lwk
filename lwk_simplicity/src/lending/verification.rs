use lending_contracts::programs::{
    issuance_factory::{IssuanceFactory, IssuanceFactoryCreation, IssuanceFactoryParameters},
    lending::LendingOffer,
    program::{MetadataProgram, SimplexProgram},
    script_auth::ScriptAuth,
};
use lending_contracts::utils::op_return_payload;
use lwk_wollet::elements::{
    confidential::Value, AssetBlindingNonce, AssetId, OutPoint, Script, Transaction, TxIn, TxOut,
};
use simplex::provider::SimplicityNetwork;

use super::LendingError;

fn check_issuance(
    txin: &TxIn,
    asset_sats: Option<u64>,
    token_sats: Option<u64>,
) -> Result<Option<AssetId>, &str> {
    let issuance = &txin.asset_issuance;

    match (asset_sats, token_sats) {
        (None, None) => {
            if !issuance.is_null() {
                return Err("input should not be an issuance input");
            }
            return Ok(None);
        }
        _ => {
            if issuance.amount.explicit() != asset_sats {
                return Err("issuance asset amount mismatch");
            }
            if issuance.inflation_keys.explicit() != token_sats {
                return Err("issuance token amount mismatch");
            }
        }
    }

    Ok(Some(txin.issuance_ids().0))
}

fn parse_explicit(out: &TxOut) -> Result<(AssetId, u64, Script), &str> {
    if !out.nonce.is_null()
        || !out.witness.rangeproof.is_empty()
        || !out.witness.surjection_proof.is_empty()
    {
        return Err("output is not explicit");
    }
    let asset = out.asset.explicit().ok_or("output asset is not explicit")?;
    let value = out.value.explicit().ok_or("output value is not explicit")?;
    let script = out.script_pubkey.clone();
    Ok((asset, value, script))
}

fn get_issuance_factory(network: &SimplicityNetwork) -> IssuanceFactory {
    let params = IssuanceFactoryParameters {
        issuing_utxos_count: 2,
        reissuance_flags: 0,
        network: *network,
    };
    IssuanceFactory::new(params)
}

fn get_prevout(tx: &Transaction, outpoint: OutPoint) -> Result<&TxOut, &str> {
    if outpoint.txid != tx.txid() {
        Err("input txid mismatch")?
    }

    tx.output
        .get(outpoint.vout as usize)
        .ok_or("input not found")
}

/// Verify change and fee outputs: they must not replicate any protocol script and, when
/// explicit, must only hold one of the allowed assets. Blinded outputs are skipped.
fn check_change_outputs<'a>(
    outputs: impl Iterator<Item = &'a TxOut>,
    forbidden_scripts: &[&Script],
    allowed_assets: &[AssetId],
) -> Result<(), &'static str> {
    for out in outputs {
        let script = &out.script_pubkey;
        if forbidden_scripts.contains(&script) || script.is_op_return() {
            return Err("unexpected change output script");
        }
        let Ok((asset, _, _)) = parse_explicit(out) else {
            continue;
        };
        if !allowed_assets.contains(&asset) {
            return Err("unexpected change output asset");
        }
    }
    Ok(())
}

pub(crate) fn parse_and_verify_issuance_factory(
    tx: &Transaction,
    expected_parameters: IssuanceFactoryParameters,
) -> Result<IssuanceFactoryCreation, LendingError> {
    let creation = IssuanceFactory::try_from_tx(tx, expected_parameters.network)?;
    let factory = &creation.factory;
    let actual_parameters = *factory.get_parameters();

    if actual_parameters.issuing_utxos_count != expected_parameters.issuing_utxos_count
        || actual_parameters.reissuance_flags != expected_parameters.reissuance_flags
    {
        return Err(LendingError::InvalidIssuanceFactory(
            "creation metadata does not match the expected factory parameters".to_string(),
        ));
    }

    let expected_factory = IssuanceFactory::new(expected_parameters);
    let factory_script = expected_factory.get_script_pubkey();
    if factory.get_script_pubkey() != factory_script {
        return Err(LendingError::InvalidIssuanceFactory(
            "factory script does not match the expected parameters".to_string(),
        ));
    }

    let factory_asset_id = creation.factory_asset_id;
    let policy_asset = expected_parameters.network.policy_asset();
    let metadata_index = IssuanceFactory::CREATION_METADATA_OUTPUT_INDEX;

    (|| -> Result<(), &str> {
        // Inputs: input 0 is the factory issuance
        let Some([ref in0]) = tx.input.get(..1) else {
            Err("too few inputs")?
        };
        if tx.input.len() > 1 {
            Err("too many inputs")?
        }

        if in0.asset_issuance.is_null() {
            Err("input 0 is not a factory issuance")?
        }
        if in0.asset_issuance.asset_blinding_nonce != AssetBlindingNonce::NEW_ISSUANCE {
            Err("factory asset is not a new issuance")?
        }
        if in0.asset_issuance.amount.explicit() != Some(2) {
            Err("factory issuance amount mismatch")?
        }
        if !matches!(
            in0.asset_issuance.inflation_keys,
            Value::Null | Value::Explicit(0)
        ) {
            Err("factory issuance must not create reissuance tokens")?
        }
        let (issued_asset, _token_asset) = in0.issuance_ids();
        if issued_asset != factory_asset_id {
            Err("factory asset does not match the issuance")?
        }

        // Outputs
        let Some([ref out0, ref out1, ref out2]) = tx.output.get(..metadata_index + 1) else {
            Err("too few outputs")?
        };

        // auth, program, metadata, change, fee
        if tx.output.len() > 5 {
            Err("too many outputs")?
        }
        if creation.auth_vout as usize != 0 || creation.program_vout as usize != 1 {
            Err("non-canonical factory output layout")?
        }

        // Output 0: program auth output, factory asset, amount 1
        let (auth_asset, auth_amount, auth_script) = parse_explicit(out0)?;
        if auth_asset != factory_asset_id
            || auth_amount != 1
            || auth_script == factory_script
            || auth_script.is_op_return()
        {
            Err("factory auth output mismatch")?
        }

        // Output 1: program output, same factory asset, amount 1
        let (program_asset, program_amount, program_script) = parse_explicit(out1)?;
        if program_asset != factory_asset_id
            || program_amount != 1
            || program_script != factory_script
        {
            Err("factory program output mismatch")?
        }

        // Output 2: OP_RETURN metadata
        let (_, metadata_value, metadata_script) = parse_explicit(out2)?;
        if !out2.is_null_data() || metadata_value != 0 || !metadata_script.is_op_return() {
            Err("missing factory creation metadata")?
        }
        let actual_metadata =
            op_return_payload(&metadata_script).ok_or("missing factory creation metadata")?;
        if actual_metadata != factory.encode_metadata_op_return().as_slice() {
            Err("factory creation metadata mismatch")?
        }

        // Change and fee outputs could only hold collateral or policy asset and must not
        // replicate any of the protocol outputs.
        check_change_outputs(
            tx.output.iter().skip(metadata_index + 1),
            &[&factory_script],
            &[policy_asset],
        )?;

        Ok(())
    })()
    .map_err(|msg| LendingError::InvalidIssuanceFactory(msg.to_string()))?;

    Ok(creation)
}

pub(crate) fn verify_issuance_factory_utxos(
    factory: &IssuanceFactory,
    factory_asset_id: AssetId,
    auth_txout: &TxOut,
    program_txout: &TxOut,
) -> Result<(), LendingError> {
    let factory_script = factory.get_script_pubkey();

    let (auth_asset, auth_amount, auth_script) = parse_explicit(auth_txout)
        .map_err(|msg| LendingError::InvalidIssuanceFactory(msg.to_string()))?;
    if auth_asset != factory_asset_id
        || auth_amount != 1
        || auth_script == factory_script
        || auth_script.is_op_return()
    {
        return Err(LendingError::InvalidIssuanceFactory(
            "factory auth UTXO does not match the verified factory".to_string(),
        ));
    }

    let (program_asset, program_amount, program_script) = parse_explicit(program_txout)
        .map_err(|msg| LendingError::InvalidIssuanceFactory(msg.to_string()))?;
    if program_asset != factory_asset_id || program_amount != 1 || program_script != factory_script
    {
        return Err(LendingError::InvalidIssuanceFactory(
            "factory program UTXO does not match the verified factory".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn parse_and_verify_lending_offer(
    tx: &Transaction,
    protocol_fee_keeper_asset_id: AssetId,
    network: SimplicityNetwork,
    auth_tx: &Transaction,
    factory_tx: &Transaction,
) -> Result<LendingOffer, LendingError> {
    const FACTORY_NFT_AMOUNT: u64 = 1;

    let policy_asset = network.policy_asset();

    // LendingOffer::try_from_tx unwraps outputs 2, 3 and 5 as explicit values, so we checking
    for index in [2, 3, 5] {
        let out = tx
            .output
            .get(index)
            .ok_or("too few outputs")
            .map_err(|msg| LendingError::InvalidLendingOffer(msg.to_string()))?;
        parse_explicit(out).map_err(|msg| LendingError::InvalidLendingOffer(msg.to_string()))?;
    }

    let offer = LendingOffer::try_from_tx(tx, protocol_fee_keeper_asset_id, network)?.offer;
    let params = offer.get_parameters();
    let script_auth = ScriptAuth::from_simplex_program(&offer);
    let factory = get_issuance_factory(&network);
    let factory_script = factory.get_script_pubkey();
    let script_auth_script = script_auth.get_script_pubkey();

    let expected_metadata = offer.encode_metadata_op_return();
    let offer_script = offer.get_script_pubkey();

    let collateral_asset = params.collateral_asset_id;
    let collateral_amount = params.offer_parameters.collateral_amount;

    let borrower_nft_expected = params.borrower_nft_asset_id;
    let lender_nft_expected = params.lender_nft_asset_id;

    (|| -> Result<(), &str> {
        // Inputs
        let Some([ref in0, ref in1, ref in2]) = tx.input.get(..3) else {
            Err("too few inputs")?
        };

        // collateral, fee funding
        if tx.input.len() > 5 {
            Err("too many inputs")?;
        }

        // Input 0: Factory auth
        // This is belongs to borrower with wallet script_pubkey, and can be spend as a regular output
        // It's have the same asset id as Factory program, and used to spend Factory program
        check_issuance(in0, None, None)?;
        let prev_out0 = get_prevout(auth_tx, in0.previous_output)?;

        let (in0_asset, in0_value, in0_script) = parse_explicit(prev_out0)?;

        // Script pubkey for this input could be any script which belongs to the borrower
        if in0_value != FACTORY_NFT_AMOUNT
            || in0_script == factory_script
            || in0_script.is_op_return()
        {
            Err("factory auth input invalid")?
        }

        // Input 1: Factory program
        // This is locked with simplicity covenant, which could be spend only if the same asset as
        // this UTXO is present in input 0 (Factory auth) and if transaction is correctly issuing a
        // new lender and borrower NFTs.
        // Contains an issue for borrower NFT.
        let borrower_nft = check_issuance(in1, Some(1), None)?.ok_or("input 1 missing issuance")?;
        let prev_out1 = get_prevout(factory_tx, in1.previous_output)?;
        let (in1_asset, in1_value, in1_script) = parse_explicit(prev_out1)?;

        if in1_script != factory_script || in1_value != FACTORY_NFT_AMOUNT {
            Err("factory program input invalid")?
        }

        // Input 2: Collateral
        // This is belongs to borrower with wallet script_pubkey, and can be spend as a regular output
        // Contains an issue for lender NFT
        let lender_nft = check_issuance(in2, Some(1), None)?.ok_or("input 1 missing issuance")?;

        // Remaining inputs should not be issuances
        for input in tx.input.iter().skip(3) {
            check_issuance(input, None, None)?;
        }

        // Outputs
        let Some([ref out0, ref out1, ref out2, ref out3, ref out4, ref out5]) = tx.output.get(..6)
        else {
            Err("too few outputs")?
        };

        // collateral change, policy asset change, fee
        if tx.output.len() > 9 {
            Err("too many outputs")?;
        }

        // Output 0: Factory auth
        // Have the same properties as Input 0
        let (out0_asset, out0_value, out0_script) = parse_explicit(out0)?;
        if out0_value != FACTORY_NFT_AMOUNT
            || out0_script == factory_script
            || out0_script.is_op_return()
        {
            Err("factory auth output invalid")?
        }

        // Output 1: Factory program
        // Have the same properties as Input 1
        let (out1_asset, out1_value, out1_script) = parse_explicit(out1)?;
        if out1_script != factory_script || out1_value != FACTORY_NFT_AMOUNT {
            Err("factory script output invalid")?
        }

        // This should be the same asset
        if out0_asset != in0_asset
            || out1_asset != in0_asset
            || out0_asset != in1_asset
            || out1_asset != in1_asset
        {
            Err("factory asset mismatch")?
        }

        // Output 2: borrower NFT
        // This is belongs to borrower with wallet script_pubkey, and can be spend as a regular output
        // Used similarly as Factory auth to claim principal after offer accepetence
        let (out2_asset, out2_value, _) = parse_explicit(out2)?;
        if out2_asset != borrower_nft_expected || out2_asset != borrower_nft || out2_value != 1 {
            Err("borrower NFT output invalid")?
        }

        // Output 3: lender NFT ScriptAuth
        // Could be claimed by the lender after offer acceptence
        let (out3_asset, out3_value, out3_script) = parse_explicit(out3)?;
        if out3_script != script_auth_script
            || out3_asset != lender_nft_expected
            || out3_asset != lender_nft
            || out3_value != 1
        {
            Err("lender NFT ScriptAuth output mismatch")?
        }

        // Output 4: OP_RETURN metadata
        // Used to reconstruct the offer info by indexer
        let (_, out4_value, out4_script) = parse_explicit(out4)?;
        if !out4.is_null_data() || out4_value != 0 || !out4_script.is_op_return() {
            Err("missing OP_RETURN metadata")?
        }

        // Recheck metadata
        let actual_metadata =
            op_return_payload(&out4_script).ok_or("missing OP_RETURN metadata")?;
        if actual_metadata != expected_metadata.as_slice() {
            Err("OP_RETURN metadata mismatch")?
        }

        // Output 5: collateral covenant
        // Main lending logic inside this covenant which could be spend after
        // cancelation/liquidation/repayment
        let (out5_asset, out5_value, out5_script) = parse_explicit(out5)?;
        if out5_script != offer_script
            || out5_asset != collateral_asset
            || out5_value != collateral_amount
        {
            Err("covenant output mismatch")?
        }

        // Change and fee outputs could only hold collateral or policy asset and must not
        // replicate any of the protocol outputs.
        check_change_outputs(
            tx.output.iter().skip(6),
            &[&factory_script, &offer_script, &script_auth_script],
            &[collateral_asset, policy_asset],
        )?;

        Ok(())
    })()
    .map_err(|msg| LendingError::InvalidLendingOffer(msg.to_string()))?;

    Ok(offer)
}

pub(crate) fn parse_and_verify_offer_acceptance(
    acceptance_tx: &Transaction,
    creation_tx: &Transaction,
    protocol_fee_keeper_asset_id: AssetId,
    network: SimplicityNetwork,
    auth_tx: &Transaction,
    factory_tx: &Transaction,
) -> Result<LendingOffer, LendingError> {
    const NFT_AMOUNT: u64 = 1;
    let policy_asset = network.policy_asset();

    let offer = parse_and_verify_lending_offer(
        creation_tx,
        protocol_fee_keeper_asset_id,
        network,
        auth_tx,
        factory_tx,
    )?;

    let params = *offer.get_parameters();
    let collateral_asset = params.collateral_asset_id;
    let collateral_amount = params.offer_parameters.collateral_amount;
    let principal_asset = params.principal_asset_id;
    let principal_amount = params.offer_parameters.principal_amount;
    let lender_nft = params.lender_nft_asset_id;

    let total_debt = params.offer_parameters.get_total_amount_to_repay();
    let active_offer = LendingOffer::new_active(params, total_debt);

    let active_offer_script = active_offer.get_script_pubkey();
    let pending_offer_script = offer.get_script_pubkey();
    let script_auth = ScriptAuth::from_simplex_program(&offer);
    let script_auth_script = script_auth.get_script_pubkey();
    let principal_auth_script = params.get_principal_output_asset_auth().get_script_pubkey();

    (|| -> Result<(), &str> {
        // Inputs
        let Some([ref in0, ref in1]) = acceptance_tx.input.get(..2) else {
            Err("too few inputs")?
        };

        // principal, fee funding
        if acceptance_tx.input.len() > 4 {
            Err("too many inputs")?;
        }

        // Input 0: pending offer covenant
        // The collateral is locked in the pending offer, spending it transitions the
        // offer to active
        check_issuance(in0, None, None)?;
        let prev_out0 = get_prevout(creation_tx, in0.previous_output)?;
        let (in0_asset, in0_value, in0_script) = parse_explicit(prev_out0)?;
        if in0_asset != collateral_asset
            || in0_value != collateral_amount
            || in0_script != pending_offer_script
        {
            Err("pending offer input invalid")?
        }

        // Input 1: lender NFT ScriptAuth
        // Unlocks the lender NFT, authorizing the spend of the pending offer
        check_issuance(in1, None, None)?;
        let prev_out1 = get_prevout(creation_tx, in1.previous_output)?;
        let (in1_asset, in1_value, in1_script) = parse_explicit(prev_out1)?;
        if in1_asset != lender_nft || in1_value != NFT_AMOUNT || in1_script != script_auth_script {
            Err("lender NFT input invalid")?
        }

        // Remaining inputs should not be issuances
        for input in acceptance_tx.input.iter().skip(2) {
            check_issuance(input, None, None)?;
        }

        // Outputs
        let Some([ref out0, ref out1, ref out2]) = acceptance_tx.output.get(..3) else {
            Err("too few outputs")?
        };

        // Principal change, fee, policy asset change
        if acceptance_tx.output.len() > 6 {
            Err("too many outputs")?;
        }

        // Output 0: active offer covenant
        // Same collateral as the pending offer, now locked with the active covenant
        let (out0_asset, out0_value, out0_script) = parse_explicit(out0)?;
        if out0_asset != collateral_asset
            || out0_value != collateral_amount
            || out0_script != active_offer_script
        {
            Err("active covenant output mismatch")?
        }

        // Output 1: principal asset auth
        // Could be claimed by the borrower with NFT
        let (out1_asset, out1_value, out1_script) = parse_explicit(out1)?;
        if out1_asset != principal_asset
            || out1_value != principal_amount
            || out1_script != principal_auth_script
        {
            Err("principal asset auth output mismatch")?
        }

        // Output 2: lender NFT
        // Returned to the lender's wallet, kept to claim the repayment later
        let (out2_asset, out2_value, out2_script) = parse_explicit(out2)?;
        if out2_asset != lender_nft
            || out2_value != NFT_AMOUNT
            || out2_script == active_offer_script
            || out2_script == pending_offer_script
            || out2_script == principal_auth_script
            || out2_script == script_auth_script
            || out2_script.is_op_return()
        {
            Err("lender NFT output invalid")?
        }

        // Change and fee outputs could only hold principal or policy asset and must not
        // replicate any of the protocol outputs.
        check_change_outputs(
            acceptance_tx.output.iter().skip(3),
            &[
                &active_offer_script,
                &pending_offer_script,
                &principal_auth_script,
                &script_auth_script,
            ],
            &[principal_asset, policy_asset],
        )?;

        Ok(())
    })()
    .map_err(|msg| LendingError::InvalidOfferAcceptance(msg.to_string()))?;

    Ok(active_offer)
}
