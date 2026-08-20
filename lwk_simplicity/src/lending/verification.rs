use lending_contracts::programs::{
    issuance_factory::{IssuanceFactory, IssuanceFactoryParameters},
    lending::LendingOffer,
    program::{MetadataProgram, SimplexProgram},
    script_auth::ScriptAuth,
};
use lending_contracts::utils::op_return_payload;
use lwk_wollet::elements::{AssetId, OutPoint, Script, Transaction, TxIn, TxOut};
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
        || out.witness.rangeproof.is_some()
        || out.witness.surjection_proof.is_some()
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

    let offer = LendingOffer::try_from_tx(tx, protocol_fee_keeper_asset_id, network)?;
    let params = offer.get_parameters();
    let script_auth = ScriptAuth::from_simplex_program(&offer);
    let factory = get_issuance_factory(&network);
    let factory_script = factory.get_script_pubkey();

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
        if out3_script != script_auth.get_script_pubkey()
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
        for out in tx.output.iter().skip(6) {
            let script = out.script_pubkey.clone();
            if script == factory_script
                || script == offer_script
                || script == script_auth.get_script_pubkey()
                || script.is_op_return()
            {
                Err("unexpected change output script")?;
            }
            let Ok((asset, _, _)) = parse_explicit(out) else {
                continue;
            };
            if asset != collateral_asset && asset != policy_asset {
                Err("unexpected change output asset")?;
            }
        }

        Ok(())
    })()
    .map_err(|msg| LendingError::InvalidLendingOffer(msg.to_string()))?;

    Ok(offer)
}
