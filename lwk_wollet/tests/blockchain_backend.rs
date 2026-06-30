use crate::test_wollet::*;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::BlockchainBackend;
use lwk_wollet::*;

fn test_has_txs_before_and_after_funding<C: BlockchainBackend>(env: TestEnv, client: C) {
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc_str = format!("ct({view_key},elwpkh({}/<0;1>/*))", signer.xpub());
    let wd: WolletDescriptor = desc_str.parse().unwrap();

    let mut wallet = TestWollet::new(client, &desc_str);

    assert!(!wallet.client.has_txs(&wd, None).unwrap());
    assert!(!wallet.client.has_txs(&wd, Some(100)).unwrap());

    let addr = wallet.wollet.address(Some(15)).unwrap().address().clone();
    wallet.fund(&env, 10000, Some(addr), None);

    assert!(wallet.client.has_txs(&wd, None).unwrap());
    assert!(wallet.client.has_txs(&wd, Some(16)).unwrap());
    assert!(!wallet.client.has_txs(&wd, Some(15)).unwrap());
}

#[test]
fn test_has_txs_before_and_after_funding_electrum() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let client = test_client_electrum(&env.electrum_url());

    test_has_txs_before_and_after_funding(env, client);
}

#[test]
fn test_has_txs_before_and_after_funding_esplora() {
    let env = TestEnvBuilder::from_env().with_esplora().build();
    let client =
        clients::blocking::EsploraClient::new(&env.esplora_url(), Network::default_regtest())
            .unwrap();

    test_has_txs_before_and_after_funding(env, client);
}

#[test]
fn test_has_txs_before_and_after_funding_waterfalls() {
    let env = TestEnvBuilder::from_env().with_waterfalls().build();

    let client =
        clients::blocking::WaterfallsClient::new(&env.waterfalls_url(), Network::default_regtest())
            .unwrap();

    test_has_txs_before_and_after_funding(env, client);
}
