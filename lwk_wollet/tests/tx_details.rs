use crate::test_wollet::*;
use lwk_test_util::*;

#[test]
fn test_tx_details() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &d);

    let txid = w.fund_btc(&env);
    let lbtc = w.policy_asset();

    let tx_det = w.wollet.tx_details(&txid).unwrap().unwrap();
    assert_eq!(tx_det.txid(), txid);
    assert_eq!(tx_det.height(), None);
    assert_eq!(tx_det.timestamp(), None);
    assert_eq!(tx_det.tx_type(), "");
    assert_eq!(tx_det.balance().len(), 0);
    assert_eq!(tx_det.fees().len(), 1);
    assert!(tx_det.fees_asset(&lbtc) > 0);
    assert_eq!(tx_det.inputs().len(), 0);
    assert_eq!(tx_det.outputs().len(), 0);
}
