use crate::test_wollet::*;
use lwk_common::Signer;
use lwk_test_util::*;
use lwk_wollet::{ElementsNetwork, WolletBuilder, WolletDescriptor};

#[test]
fn test_single_address_tr() {
    // Monitor a wallet that consists in a single taproot address
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let view_key = generate_view_key();
    let pk = "0202020202020202020202020202020202020202020202020202020202020202";

    let desc = format!("ct({view_key},eltr({pk}))");
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    w.fund_btc(&env);
    let balance = w.balance_btc();
    assert!(balance > 0);
    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);

    // Receive unconfidential / explicit
    let satoshi = 5_000;
    w.fund_explicit(&env, satoshi, None, None);
    assert_eq!(w.balance_btc(), balance);

    let explicit_utxos = w.wollet.explicit_utxos().unwrap();
    assert_eq!(explicit_utxos.len(), 1);

    // Signing is not supported
    let mut pset = w
        .tx_builder()
        .add_lbtc_recipient(&w.address(), 1000)
        .unwrap()
        .finish()
        .unwrap();

    let signer = generate_signer();
    let sigs_added = signer.sign(&mut pset).unwrap();
    assert_eq!(sigs_added, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_address_tr_async() {
    let env = TestEnvBuilder::from_env().with_esplora().build();

    let view_key = generate_view_key();
    let pk = "0202020202020202020202020202020202020202020202020202020202020202";

    let desc = format!("ct({view_key},eltr({pk}))");
    let mut client = lwk_wollet::asyncr::EsploraClient::new(
        ElementsNetwork::default_regtest(),
        &env.esplora_url(),
    );
    let network = ElementsNetwork::default_regtest();
    let descriptor: WolletDescriptor = desc.parse().unwrap();
    let mut wollet = WolletBuilder::new(network, descriptor).build().unwrap();

    let addr = wollet.address(None).unwrap();

    let _txid = env.elementsd_sendtoaddress(addr.address(), 2_000_011, None);

    // TODO: wait_update_with_txs is not working correctly in this case. It seems
    // it returns even if the tx is not yet in the wallet, fix it and remove this unconditional wait
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let update = client.full_scan(&wollet).await.unwrap().unwrap();

    wollet.apply_update(update).unwrap();

    let balance = *wollet
        .balance()
        .unwrap()
        .get(&regtest_policy_asset())
        .unwrap();
    assert!(balance > 0);
    let utxos = wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);
}
