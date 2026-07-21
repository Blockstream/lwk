use crate::test_wollet::*;
use elements::Txid;
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

    // Tx list without txs
    let txsopt = TxsOpt {
        without_tx: true,
        ..Default::default()
    };
    let txs_minimal = w.wollet.txs(&txsopt).unwrap();
    assert_eq!(txs_minimal.len(), 2);
    assert!(txs_minimal.iter().any(|tx| tx.txid() == txid1));
    assert!(txs_minimal.iter().any(|tx| tx.txid() == txid2));
    assert!(txs_minimal.iter().all(|tx| tx.tx().is_none()));
    assert!(txs_minimal.iter().all(|tx| tx.tx_type().is_empty()));
    assert!(txs_minimal.iter().all(|tx| tx.balance().is_empty()));
    assert!(txs_minimal.iter().all(|tx| tx.fees().is_empty()));
    assert!(txs_minimal.iter().all(|tx| tx.inputs().is_empty()));
    assert!(txs_minimal.iter().all(|tx| tx.outputs().is_empty()));
}

#[test]
fn test_tx_details_no_wildcard() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &d);
    let signers = [&AnySigner::Software(s.clone())];

    let txid1 = w.fund_btc(&env);
    let txid2 = w.send_btc(&signers, None, None);

    let txopt = TxOpt::default();
    let tx_det1 = w.wollet.tx_details(&txid1, &txopt).unwrap().unwrap();
    let tx_det2 = w.wollet.tx_details(&txid2, &txopt).unwrap().unwrap();

    assert!(!tx_det1.inputs()[0].is_mine());
    assert!(tx_det1.outputs().iter().any(|o| o.is_mine()));

    assert!(tx_det2.inputs()[0].is_mine());
    assert!(tx_det2.outputs().iter().any(|o| o.is_mine()));
    assert!(tx_det2.outputs().iter().any(|o| !o.is_mine()));
}

#[test]
fn test_txs_cannot_unblind() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();

    let view_key1 = generate_view_key();
    let d1 = format!("ct({view_key1},elwpkh({}/*))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w1 = TestWollet::new(client, &d1);

    let view_key2 = generate_view_key();
    let d2 = format!("ct({view_key2},elwpkh({}/*))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w2 = TestWollet::new(client, &d2);

    let txid1 = w1.fund_btc(&env);
    let txid2 = w2.fund_btc(&env);
    w1.wait_for_tx_outside_list(&txid2);
    w2.wait_for_tx_outside_list(&txid1);

    assert_eq!(w1.wollet.transactions().unwrap().len(), 1);
    assert_eq!(w2.wollet.transactions().unwrap().len(), 1);
    let opt = TxsOpt::default();
    assert_eq!(w1.wollet.txs(&opt).unwrap().len(), 1);
    assert_eq!(w2.wollet.txs(&opt).unwrap().len(), 1);

    // Simulate an app restart
    let descriptor = w1.wollet.wollet_descriptor();
    let path = w1.path();
    let network = Network::default_regtest();
    let reloaded = WolletBuilder::new(network, descriptor)
        .with_legacy_fs_store(&path)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(reloaded.txs(&opt).unwrap().len(), 1);

    let with_cannot_unblind_opt = TxsOpt {
        with_cannot_unblind: true,
        ..Default::default()
    };
    assert_eq!(reloaded.txs(&with_cannot_unblind_opt).unwrap().len(), 2);
}

#[test]
fn test_txs_pagination_with_cannot_unblind() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let s = generate_signer();

    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &d);

    // Foreign wallet sharing the same xpub (hence the same scripts) but a different view
    // key, used to fund `w`'s addresses with outputs `w` downloads but cannot unblind.
    let foreign_view_key = generate_view_key();
    let foreign_d = format!("ct({foreign_view_key},elwpkh({}/*))", s.xpub());
    let foreign_client = test_client_electrum(&env.electrum_url());
    let mut foreign = TestWollet::new(foreign_client, &foreign_d);

    let mut height = w.wollet.tip().height();

    fn gen_block(height: &mut u32, w: &mut TestWollet<ElectrumClient>, env: &TestEnv) {
        env.elementsd_generate(1);
        *height += 1;
        w.wait_height(*height);
    }

    let txid10 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    let txid9 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    let txid8 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    let txid7 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    // txid6 cannot unblind
    let txid6 = foreign.fund_btc(&env);
    w.wait_for_tx_outside_list(&txid6);
    gen_block(&mut height, &mut w, &env);
    let txid5 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    let txid4 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    let txid3 = w.fund_btc(&env);
    gen_block(&mut height, &mut w, &env);
    // txid2 cannot unblind
    let txid2 = foreign.fund_btc(&env);
    w.wait_for_tx_outside_list(&txid2);
    gen_block(&mut height, &mut w, &env);
    let txid1 = w.fund_btc(&env);

    let opt = TxsOpt::default();
    assert_eq!(w.wollet.txs(&opt).unwrap().len(), 8);

    let with_cannot_unblind_opt = TxsOpt {
        with_cannot_unblind: true,
        ..Default::default()
    };
    assert_eq!(w.wollet.txs(&with_cannot_unblind_opt).unwrap().len(), 10);

    let page_size = 3usize;
    let page = |n| -> Vec<Txid> {
        let offset = n * page_size;
        w.wollet
            .txs(&TxsOpt {
                offset: Some(offset),
                limit: Some(page_size),
                ..Default::default()
            })
            .unwrap()
            .iter()
            .map(|t| t.txid())
            .collect()
    };

    assert_eq!(page(0), vec![txid1, txid3, txid4]);
    assert_eq!(page(1), vec![txid5, txid7, txid8]);
    assert_eq!(page(2), vec![txid9, txid10]);
}
