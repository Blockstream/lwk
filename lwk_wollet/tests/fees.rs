use crate::test_wollet::*;
use lwk_signer::AnySigner;
use lwk_test_util::*;
use lwk_wollet::elements::{
    pset::{Input, Output, PartiallySignedTransaction},
    secp256k1_zkp::{RangeProof, SurjectionProof},
    {BlindAssetProofs, BlindValueProofs},
};
use lwk_wollet::*;
use std::collections::HashMap;

fn fee_out(satoshi: u64, asset: elements::AssetId) -> Output {
    Output::new_explicit(elements::Script::default(), satoshi, asset, None)
}

fn wallet_output(wollet: &Wollet, satoshi: u64, asset: elements::AssetId) -> Output {
    let address = wollet.address(None).unwrap().address().clone();
    let script_pubkey = address.script_pubkey();
    let blinding_key = Some(elements::bitcoin::PublicKey::new(
        address.blinding_pubkey.unwrap(),
    ));
    Output {
        script_pubkey,
        blinding_key,
        amount: Some(satoshi),
        asset: Some(asset),
        blinder_index: Some(0),
        ..Default::default()
    }
}

fn pset_raw(
    wollet: &Wollet,
    utxos: &[WalletTxOut],
    outputs: &[Output],
) -> PartiallySignedTransaction {
    let mut pset = PartiallySignedTransaction::new_v2();
    let mut inp_txout_sec = HashMap::new();
    let mut balance = HashMap::new();
    let mut rng = rand::thread_rng();

    // Add inputs
    for (idx, utxo) in utxos.iter().enumerate() {
        *balance.entry(utxo.unblinded.asset).or_insert(0) += utxo.unblinded.value;
        inp_txout_sec.insert(idx, utxo.unblinded);

        let txout = &wollet
            .transaction(&utxo.outpoint.txid)
            .unwrap()
            .unwrap()
            .tx
            .output[utxo.outpoint.vout as usize];
        let mut input = Input::from_prevout(utxo.outpoint);

        input.asset = Some(utxo.unblinded.asset);
        input.amount = Some(utxo.unblinded.value);
        input.blind_asset_proof = Some(Box::new(
            SurjectionProof::blind_asset_proof(
                &mut rng,
                &EC,
                utxo.unblinded.asset,
                utxo.unblinded.asset_bf,
            )
            .unwrap(),
        ));
        input.blind_value_proof = Some(Box::new(
            RangeProof::blind_value_proof(
                &mut rng,
                &EC,
                utxo.unblinded.value,
                txout.value.commitment().unwrap(),
                txout.asset.commitment().unwrap(),
                utxo.unblinded.value_bf,
            )
            .unwrap(),
        ));

        // Skip adding rangeproof
        input.witness_utxo = Some(txout.clone());

        pset.add_input(input);
    }

    // Add outputs
    for output in outputs {
        pset.add_output(output.clone())
    }

    // Blind
    pset.blind_last(&mut rng, &EC, &inp_txout_sec).unwrap();

    // Add information for signers
    wollet.add_details(&mut pset).unwrap();

    pset
}

#[test]
fn test_fees() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    // Sender
    let s1 = generate_signer();
    let view_key = generate_view_key();
    let d1 = format!("ct({view_key},elwpkh({}/*))", s1.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut w1 = TestWollet::new(client, &d1);

    // Receiver
    let s2 = generate_signer();
    let view_key = generate_view_key();
    let d2 = format!("ct({view_key},elwpkh({}/*))", s2.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut w2 = TestWollet::new(client, &d2);

    // Fund wallet
    w1.fund_btc(&env);
    let lbtc = w1.policy_asset();

    // Fee LBTC (std transaction)
    let utxos = w1.wollet.utxos().unwrap();
    let b = w1.balance(&lbtc);
    let outputs = vec![
        wallet_output(&w2.wollet, 1_000, lbtc),
        wallet_output(&w1.wollet, b - 2_000, lbtc),
        fee_out(1_000, lbtc),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 1_000);

    // No fee output
    let utxos = w1.wollet.utxos().unwrap();
    let b = w1.balance(&lbtc);
    let outputs = vec![
        wallet_output(&w2.wollet, 1_000, lbtc),
        wallet_output(&w1.wollet, b - 1_000, lbtc),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 0);

    // Fee not last output
    let utxos = w1.wollet.utxos().unwrap();
    let b = w1.balance(&lbtc);
    let outputs = vec![
        fee_out(1_000, lbtc),
        wallet_output(&w2.wollet, 1_000, lbtc),
        wallet_output(&w1.wollet, b - 2_000, lbtc),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 1_000);

    // Multiple fee outputs
    let utxos = w1.wollet.utxos().unwrap();
    let b = w1.balance(&lbtc);
    let outputs = vec![
        wallet_output(&w2.wollet, 1_000, lbtc),
        wallet_output(&w1.wollet, b - 3_000, lbtc),
        fee_out(1_000, lbtc),
        fee_out(1_000, lbtc),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 2_000);

    // Fee paid in asset
    // TODO: show asset fees correctly (require interface changes)
    let (asset, token) = w1.issueasset(&[&AnySigner::Software(s1.clone())], 10, 1, None, None);

    let utxos = w1.wollet.utxos().unwrap();
    let b = w1.balance(&lbtc);
    let outputs = vec![
        wallet_output(&w1.wollet, b, lbtc),
        wallet_output(&w2.wollet, 1, asset),
        wallet_output(&w1.wollet, 8, asset),
        wallet_output(&w1.wollet, 1, token),
        fee_out(1, asset),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 1);

    // Multiple asset fee outputs
    let utxos = w1.wollet.utxos().unwrap();
    let outputs = vec![
        wallet_output(&w1.wollet, b, lbtc),
        wallet_output(&w2.wollet, 1, asset),
        wallet_output(&w1.wollet, 5, asset),
        wallet_output(&w1.wollet, 1, token),
        fee_out(1, asset),
        fee_out(1, asset),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 2);

    // Multiple fee outputs with different assets
    let utxos = w1.wollet.utxos().unwrap();
    let outputs = vec![
        wallet_output(&w1.wollet, b - 1_000, lbtc),
        wallet_output(&w2.wollet, 1, asset),
        wallet_output(&w1.wollet, 3, asset),
        wallet_output(&w1.wollet, 1, token),
        fee_out(1_000, lbtc),
        fee_out(1, asset),
    ];
    let mut pset = pset_raw(&w1.wollet, &utxos, &outputs);
    w1.sign(&s1, &mut pset);
    let txid = w1.send(&mut pset);
    w2.sync();
    let tx = w2.wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.fee, 1_001);
}
