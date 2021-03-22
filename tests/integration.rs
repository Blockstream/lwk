use bewallet::model::SPVVerifyResult;
use std::env;

mod test_session;

#[test]
fn liquid() {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC")
        .expect("env ELECTRS_LIQUID_EXEC pointing to electrs executable is required");
    let node_exec = env::var("ELEMENTSD_EXEC")
        .expect("env ELEMENTSD_EXEC pointing to elementsd executable is required");
    let debug = env::var("DEBUG").is_ok();

    let mut test_electrum_wallet = test_session::setup_wallet(true, debug, electrs_exec, node_exec);

    let node_address = test_electrum_wallet.node_getnewaddress(Some("p2sh-segwit"));
    let node_bech32_address = test_electrum_wallet.node_getnewaddress(Some("bech32"));
    let node_legacy_address = test_electrum_wallet.node_getnewaddress(Some("legacy"));

    test_electrum_wallet.fund_btc();
    let asset = test_electrum_wallet.fund_asset();

    let txid = test_electrum_wallet.send_tx(&node_address, 10_000, None, None);
    test_electrum_wallet.send_tx_to_unconf();
    test_electrum_wallet.is_verified(&txid, SPVVerifyResult::InProgress);
    test_electrum_wallet.send_tx(&node_bech32_address, 1_000, None, None);
    test_electrum_wallet.send_tx(&node_legacy_address, 1_000, None, None);
    test_electrum_wallet.send_tx(&node_address, 1_000, Some(asset.clone()), None);
    test_electrum_wallet.send_tx(&node_address, 100, Some(asset.clone()), None); // asset should send below dust limit
    test_electrum_wallet.send_all(&node_address, Some(asset.clone()));
    test_electrum_wallet.send_all(&node_address, test_electrum_wallet.policy_asset());
    test_electrum_wallet.mine_block();
    test_electrum_wallet.fund_btc();
    let asset1 = test_electrum_wallet.fund_asset();
    let asset2 = test_electrum_wallet.fund_asset();
    let asset3 = test_electrum_wallet.fund_asset();
    let assets = vec![asset1, asset2, asset3];
    test_electrum_wallet.send_multi(3, 1_000, &vec![]);
    test_electrum_wallet.send_multi(10, 1_000, &assets);
    test_electrum_wallet.mine_block();
    test_electrum_wallet.create_fails();
    test_electrum_wallet.is_verified(&txid, SPVVerifyResult::Verified);
    let utxos = test_electrum_wallet.utxos();
    test_electrum_wallet.send_tx(&node_address, 1_000, None, Some(utxos));

    test_electrum_wallet.stop();
}
