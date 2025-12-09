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
    assert!(w.balance_btc() > 0);
    assert!(w.wollet.utxos().unwrap().len() > 0);
}
