use bewallet::SPVVerifyResult;
use std::env;

mod test_session;

#[test]
fn liquid() {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC")
        .expect("env ELECTRS_LIQUID_EXEC pointing to electrs executable is required");
    let node_exec = env::var("ELEMENTSD_EXEC")
        .expect("env ELEMENTSD_EXEC pointing to elementsd executable is required");
    let debug = env::var("DEBUG").is_ok();

    let mut server = test_session::TestElectrumServer::new(debug, electrs_exec, node_exec);
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
    let mut wallet = test_session::TestElectrumWallet::new(&server.electrs.electrum_url, mnemonic);

    let node_address = server.node_getnewaddress(Some("p2sh-segwit"));
    let node_bech32_address = server.node_getnewaddress(Some("bech32"));
    let node_legacy_address = server.node_getnewaddress(Some("legacy"));

    wallet.fund_btc(&mut server);
    let asset = wallet.fund_asset(&mut server);

    let txid = wallet.send_tx(&node_address, 10_000, None, None);
    wallet.send_tx_to_unconf(&mut server);
    wallet.is_verified(&txid, SPVVerifyResult::InProgress);
    wallet.send_tx(&node_bech32_address, 1_000, None, None);
    wallet.send_tx(&node_legacy_address, 1_000, None, None);
    wallet.send_tx(&node_address, 1_000, Some(asset.clone()), None);
    wallet.send_tx(&node_address, 100, Some(asset.clone()), None); // asset should send below dust limit
    wallet.wait_for_block(server.mine_block());
    let asset1 = wallet.fund_asset(&mut server);
    let asset2 = wallet.fund_asset(&mut server);
    let asset3 = wallet.fund_asset(&mut server);
    let assets = vec![asset1, asset2, asset3];
    wallet.send_multi(3, 1_000, &vec![], &mut server);
    wallet.send_multi(10, 1_000, &assets, &mut server);
    wallet.wait_for_block(server.mine_block());
    wallet.create_fails(&mut server);
    wallet.is_verified(&txid, SPVVerifyResult::Verified);
    let utxos = wallet.utxos();
    wallet.send_tx(&node_address, 1_000, None, Some(utxos));

    server.stop();
}

#[test]
fn dex() {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC")
        .expect("env ELECTRS_LIQUID_EXEC pointing to electrs executable is required");
    let node_exec = env::var("ELEMENTSD_EXEC")
        .expect("env ELEMENTSD_EXEC pointing to elementsd executable is required");
    let debug = env::var("DEBUG").is_ok();

    let mut server = test_session::TestElectrumServer::new(debug, electrs_exec, node_exec);

    let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
    let mut taker = test_session::TestElectrumWallet::new(&server.electrs.electrum_url, mnemonic1);

    taker.fund_btc(&mut server);
    let asset1 = taker.fund_asset(&mut server);

    // asset db tests
    taker.liquidex_assets_db_roundtrip();

    let mnemonic2 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon actual".to_string();
    let mut maker = test_session::TestElectrumWallet::new(&server.electrs.electrum_url, mnemonic2);

    let asset2 = maker.fund_asset(&mut server);

    assert_eq!(taker.balance(&asset1), 10_000);
    assert_eq!(taker.balance(&asset2), 0);
    assert_eq!(maker.balance(&asset1), 0);
    assert_eq!(maker.balance(&asset2), 10_000);

    // asset2 10_000 <-> asset1 10_000 (no change)
    maker.liquidex_add_asset(&asset1);
    let utxo = maker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &asset1, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    assert_eq!(taker.balance(&asset1), 0);
    assert_eq!(taker.balance(&asset2), 10_000);
    assert_eq!(maker.balance(&asset1), 10_000);
    assert_eq!(maker.balance(&asset2), 0);

    // asset1 10_000 <-> asset2 5_000 (maker creates change)
    maker.liquidex_add_asset(&asset2);
    let utxo = maker.asset_utxos(&asset1)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &asset2, 0.5);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    assert_eq!(taker.balance(&asset1), 10_000);
    assert_eq!(taker.balance(&asset2), 5_000);
    assert_eq!(maker.balance(&asset1), 0);
    assert_eq!(maker.balance(&asset2), 5_000);

    // asset2 5_000 <-> L-BTC 5_000
    let policy_asset = taker.policy_asset();
    let sats_w1_policy_before = taker.balance(&policy_asset);
    maker.liquidex_add_asset(&policy_asset);
    let utxo = maker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &policy_asset, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    let sats_w1_policy_after = taker.balance(&policy_asset);
    assert_eq!(sats_w1_policy_before - sats_w1_policy_after - fee, 5_000);
    assert_eq!(taker.balance(&asset2), 10_000);
    assert_eq!(maker.balance(&asset2), 0);
    assert_eq!(maker.balance(&policy_asset), 5_000);

    // L-BTC 5_000 <-> L-BTC 10_000
    let sats_w1_policy_before = taker.balance(&policy_asset);
    let utxo = maker.asset_utxos(&policy_asset)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &policy_asset, 2.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    let sats_w1_policy_after = taker.balance(&policy_asset);
    assert_eq!(sats_w1_policy_before - sats_w1_policy_after - fee, 5_000);
    assert_eq!(maker.balance(&policy_asset), 10_000);

    // L-BTC 10_000 <-> asset2 5_000
    let sats_w1_policy_before = taker.balance(&policy_asset);
    let utxo = maker.asset_utxos(&policy_asset)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &asset2, 0.5);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    let sats_w1_policy_after = taker.balance(&policy_asset);
    assert_eq!(sats_w1_policy_after - sats_w1_policy_before + fee, 10_000);
    assert_eq!(taker.balance(&asset2), 5_000);
    assert_eq!(maker.balance(&asset2), 5_000);
    assert_eq!(maker.balance(&policy_asset), 0);

    // asset2 5_000 <-> asset2 5_000
    let utxo = maker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = maker.liquidex_make(&utxo, &asset2, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);
    maker.wait_for_tx(&txid);

    assert_eq!(taker.balance(&asset2), 5_000);
    assert_eq!(maker.balance(&asset2), 5_000);

    // swaps within the same wallet
    assert_eq!(taker.balance(&asset1), 10_000);
    assert_eq!(taker.balance(&asset2), 5_000);
    let balance_btc_0 = taker.balance(&policy_asset);
    assert!(balance_btc_0 > 0);

    // asset2 5_000 <-> asset1 10_000 (no change)
    taker.liquidex_add_asset(&asset1);
    let utxo = taker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = taker.liquidex_make(&utxo, &asset1, 2.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    assert_eq!(taker.balance(&asset1), 10_000);
    assert_eq!(taker.balance(&asset2), 5_000);
    let balance_btc_1 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_1, balance_btc_0 - fee);

    // asset2 5_000 <-> asset1 5_000 (change)
    let utxo = taker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = taker.liquidex_make(&utxo, &asset1, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    assert_eq!(taker.balance(&asset1), 10_000);
    assert_eq!(taker.balance(&asset2), 5_000);
    let balance_btc_2 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_2, balance_btc_1 - fee);

    // asset2 5_000 <-> L-BTC 5_000
    taker.liquidex_add_asset(&policy_asset);
    let utxo = taker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = taker.liquidex_make(&utxo, &policy_asset, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    assert_eq!(taker.balance(&asset2), 5_000);
    let balance_btc_3 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_3, balance_btc_2 - fee);

    // L-BTC <-> L-BTC
    let utxos = taker.asset_utxos(&policy_asset);
    let utxo = if utxos[1].unblinded.value > utxos[0].unblinded.value {
        utxos[0].txo.outpoint
    } else {
        utxos[1].txo.outpoint
    };
    let proposal = taker.liquidex_make(&utxo, &policy_asset, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    let balance_btc_4 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_4, balance_btc_3 - fee);

    // asset2 5_000 <-> asset2 5_000
    taker.liquidex_add_asset(&asset2);
    let utxo = taker.asset_utxos(&asset2)[0].txo.outpoint;
    let proposal = taker.liquidex_make(&utxo, &policy_asset, 1.0);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    assert_eq!(taker.balance(&asset2), 5_000);
    let balance_btc_5 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_5, balance_btc_4 - fee);

    // L-BTC <-> asset2 5_000
    let utxo = taker.asset_utxos(&policy_asset)[0].clone();
    let sats = utxo.unblinded.value;
    let utxo = utxo.txo.outpoint;
    let rate = 5_000.0 / sats as f64;
    let proposal = taker.liquidex_make(&utxo, &policy_asset, rate);

    let txid = taker.liquidex_take(&proposal);
    taker.wait_for_tx(&txid);

    let fee = taker.get_fee(&txid);
    let balance_btc_6 = taker.balance(&policy_asset);
    assert_eq!(balance_btc_6, balance_btc_5 - fee);

    server.stop();
}
