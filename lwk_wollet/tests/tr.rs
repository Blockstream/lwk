use crate::test_wollet::*;
use lwk_test_util::*;

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
    assert_eq!(explicit_utxos.len(), 1)
}
