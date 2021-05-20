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
    let mut wallet = test_session::TestElectrumWallet::new(&server.electrs_url, mnemonic);

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
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
    let mut wallet = test_session::TestElectrumWallet::new(&server.electrs_url, mnemonic);

    wallet.fund_btc(&mut server);
    wallet.fund_asset(&mut server);

    wallet.liquidex_assets();
    wallet.liquidex_roundtrip();

    server.stop();
}
