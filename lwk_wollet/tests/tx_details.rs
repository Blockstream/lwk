use crate::test_wollet::*;
use lwk_signer::*;
use lwk_test_util::*;
use lwk_wollet::*;

#[test]
fn test_tx_details() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &d);
    let signers = [&AnySigner::Software(s.clone())];

    let txid1 = w.fund_btc(&env);
    let txid2 = w.send_btc(&signers, None, None);
    let lbtc = w.policy_asset();

    let txopt = TxOpt::default();
    let tx_det = w.wollet.tx_details(&txid1, &txopt).unwrap().unwrap();
    assert_eq!(tx_det.txid(), txid1);
    assert_eq!(tx_det.height(), None);
    assert_eq!(tx_det.timestamp(), None);
    assert_eq!(tx_det.tx_type(), "incoming");
    let balance = tx_det.balance();
    assert_eq!(balance.len(), 1);
    assert_eq!(*balance.get(&lbtc).unwrap(), 1_000_000i64);
    assert_eq!(tx_det.fees().len(), 1);
    let fee_sats = tx_det.fees_asset(&lbtc);
    assert!(fee_sats > 0);
    let inputs = tx_det.inputs();
    let outputs = tx_det.outputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(outputs.len(), 3);

    let input = &inputs[0];
    assert_ne!(input.outpoint().txid, txid1);
    assert!(input.script_pubkey().is_none());
    assert!(input.height().is_none());
    assert!(input.path().is_none());
    assert!(input.address().is_none());
    assert!(!input.is_explicit());
    assert!(input.is_spent());

    let out_recv = outputs.iter().find(|o| o.path().is_some()).unwrap();
    assert_eq!(out_recv.outpoint().txid, txid1);
    assert!(out_recv.height().is_none());
    assert_eq!(out_recv.path().unwrap().len(), 2);
    let address = out_recv.address().unwrap();
    assert_eq!(&address.script_pubkey(), out_recv.script_pubkey().unwrap());
    assert!(address.blinding_pubkey.is_some());
    assert!(!out_recv.is_explicit());
    assert_eq!(out_recv.unblinded().unwrap().asset, lbtc);
    assert_eq!(out_recv.unblinded().unwrap().value, 1_000_000);
    assert!(out_recv.is_spent());

    let out_change = outputs.iter().find(|o| o.path().is_none()).unwrap();
    assert_eq!(out_change.outpoint().txid, txid1);
    assert_ne!(out_change.outpoint().vout, out_recv.outpoint().vout);
    assert!(out_change.height().is_none());
    assert!(out_change.path().is_none());
    let address = out_change.address().unwrap();
    assert_eq!(
        &address.script_pubkey(),
        out_change.script_pubkey().unwrap()
    );
    assert!(address.blinding_pubkey.is_none());
    assert!(!out_change.is_explicit());
    assert!(out_change.unblinded().is_none());
    assert!(!out_change.is_spent());

    let out_fee = &outputs[2];
    assert_eq!(out_fee.outpoint().txid, txid1);
    assert_eq!(out_fee.outpoint().vout, 2);
    assert!(out_fee.script_pubkey().unwrap().is_empty());
    assert!(out_fee.height().is_none());
    assert!(out_fee.path().is_none());
    assert!(out_fee.address().is_none());
    assert!(out_fee.is_explicit());
    assert_eq!(out_fee.unblinded().unwrap().asset, lbtc);
    assert_eq!(out_fee.unblinded().unwrap().value, fee_sats);
    assert!(!out_fee.is_spent());

    let tx_det = w.wollet.tx_details(&txid2, &txopt).unwrap().unwrap();
    assert_eq!(tx_det.txid(), txid2);
    assert_eq!(tx_det.height(), None);
    assert_eq!(tx_det.timestamp(), None);
    assert_eq!(tx_det.tx_type(), "redeposit");
    let balance = tx_det.balance();
    assert_eq!(balance.len(), 1);
    assert_eq!(tx_det.fees().len(), 1);
    let fee_sats = tx_det.fees_asset(&lbtc);
    assert!(fee_sats > 0);
    assert_eq!(*balance.get(&lbtc).unwrap(), -(fee_sats as i64));
    let inputs = tx_det.inputs();
    let outputs = tx_det.outputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(outputs.len(), 3);

    let input = &inputs[0];
    assert_eq!(input.outpoint().txid, txid1);
    assert_eq!(input.outpoint().vout, out_recv.outpoint().vout);
    assert!(input.height().is_none());
    assert_eq!(
        input.script_pubkey().unwrap(),
        out_recv.script_pubkey().unwrap()
    );
    assert_eq!(input.path().unwrap(), out_recv.path().unwrap());
    assert_eq!(input.address().unwrap(), out_recv.address().unwrap());
    assert!(!input.is_explicit());
    assert_eq!(input.unblinded().unwrap(), out_recv.unblinded().unwrap());
    assert!(input.is_spent());

    let mut txsopt = TxsOpt::default();
    let txs = w.wollet.txs(&txsopt).unwrap();
    assert_eq!(txs.len(), 2);
    // Both are unconfirmed, so order depends on txid, which is random
    assert!(txs.iter().any(|tx| tx.txid() == txid1));
    assert!(txs.iter().any(|tx| tx.txid() == txid2));

    txsopt.limit = Some(1);
    let txs1 = w.wollet.txs(&txsopt).unwrap();
    assert_eq!(txs1.len(), 1);
    assert_eq!(txs[0], txs1[0]);

    txsopt.offset = Some(1);
    txsopt.limit = None;
    let txs2 = w.wollet.txs(&txsopt).unwrap();
    assert_eq!(txs2.len(), 1);
    assert_eq!(txs[1], txs2[0]);
}
