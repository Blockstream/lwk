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

    wallet.fund_btc(&mut server);
    let _asset = wallet.fund_asset(&mut server);

    server.stop();
}
