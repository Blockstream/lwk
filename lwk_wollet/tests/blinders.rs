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

    let txid1 = w.fund_btc(&env);
    let lbtc = w.policy_asset();
    assert_eq!(w.wollet.all_unblinded().len(), 1);

    let node_addr = env.elementsd_getnewaddress();
    let buildtx = w
        .tx_builder()
        .add_recipient(&node_addr, 10_000, lbtc)
        .unwrap()
        .build()
        .unwrap();

    // Generate an update that allows us to persist the blinders
    let update = buildtx.update(&w.wollet).unwrap();
    w.wollet.apply_update(update).unwrap();

    // all_unblinded returns the new blinders even now
    let txid2 = buildtx.pset().extract_tx().unwrap().txid();
    let unblinded = w.wollet.all_unblinded();
    assert_eq!(unblinded.len(), 3);
    assert_eq!(unblinded.keys().filter(|op| op.txid == txid1).count(), 1);
    assert_eq!(unblinded.keys().filter(|op| op.txid == txid2).count(), 2);

    let mut pset = buildtx.pset().clone();
    w.sign(&s, &mut pset);
    let txid = w.send(&mut pset);
    let tx = w
        .wollet
        .tx_details(&txid, &TxOpt::default())
        .unwrap()
        .unwrap();
    // sent output
    assert!(tx.outputs()[0].unblinded().is_some());
    assert!(!tx.outputs()[0].is_explicit());
    // change
    assert!(tx.outputs()[1].unblinded().is_some());
    assert!(!tx.outputs()[1].is_explicit());
    // fee
    assert!(tx.outputs()[2].unblinded().is_some());
    assert!(tx.outputs()[2].is_explicit());
}
