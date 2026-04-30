use crate::test_wollet::*;
use lwk_test_util::*;
use lwk_wollet::*;

#[test]
fn test_build_tx() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", s.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    w.fund_btc(&env);
    let lbtc = w.policy_asset();

    let node_addr = env.elementsd_getnewaddress();
    let buildtx = w
        .tx_builder()
        .add_recipient(&node_addr, 10_000, lbtc)
        .unwrap()
        .build()
        .unwrap();
    let mut pset = buildtx.pset().clone();
    w.sign(&s, &mut pset);
    let txid = w.send(&mut pset);
    let tx = w
        .wollet
        .tx_details(&txid, &TxOpt::default())
        .unwrap()
        .unwrap();
    // sent output
    assert!(tx.outputs()[0].unblinded().is_none());
    assert!(!tx.outputs()[0].is_explicit());
    // change
    assert!(tx.outputs()[1].unblinded().is_some());
    assert!(!tx.outputs()[1].is_explicit());
    // fee
    assert!(tx.outputs()[2].unblinded().is_some());
    assert!(tx.outputs()[2].is_explicit());
}
