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

#[test]
fn test_scan_unchanged() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, TEST_DESCRIPTOR);
    assert!(wallet.client.full_scan(&wallet.wollet).unwrap().is_none());
    assert!(wallet
        .client
        .full_scan(&wallet.wollet.state())
        .unwrap()
        .is_none());

    let update = wallet
        .client
        .full_scan_to_index(&wallet.wollet, 40)
        .unwrap()
        .unwrap();
    wallet.wollet.apply_update(update).unwrap();
    assert!(wallet
        .client
        .full_scan_to_index(&wallet.wollet, 40)
        .unwrap()
        .is_none());

    let txid = env.elementsd_sendtoaddress(&wallet.address(), 10_000, None);
    let update = crate::wait_blockchain_tx_update(&mut wallet.client, &wallet.wollet);
    assert!(!update.new_txs.txs.is_empty());
    wallet.wollet.apply_update(update).unwrap();
    assert_eq!(
        wallet.wollet.transaction(&txid).unwrap().unwrap().height,
        None
    );

    env.elementsd_generate(1);
    let mut update = crate::wait_blockchain_tx_update(&mut wallet.client, &wallet.wollet);
    assert!(update.new_txs.txs.is_empty());
    assert!(update
        .txid_height_new
        .iter()
        .any(|(id, height)| *id == txid && height.is_some()));
    // Model a tip arriving before the transaction's confirmation history.
    update.txid_height_new.clear();
    wallet.wollet.apply_update(update).unwrap();
    let update = wallet.client.full_scan(&wallet.wollet).unwrap().unwrap();
    assert!(update.new_txs.txs.is_empty());
    wallet.wollet.apply_update(update).unwrap();
    assert!(wallet
        .wollet
        .transaction(&txid)
        .unwrap()
        .unwrap()
        .height
        .is_some());
    assert!(wallet.client.full_scan(&wallet.wollet).unwrap().is_none());

    // Missing timestamps are repaired even for already-known transaction heights.
    let mut restored =
        WolletBuilder::new(Network::default_regtest(), TEST_DESCRIPTOR.parse().unwrap())
            .build()
            .unwrap();
    let mut snapshot = wallet.client.full_scan(&restored).unwrap().unwrap();
    let confirmed_tip = snapshot.tip.clone();
    snapshot.timestamps.clear();
    restored.apply_update(snapshot.clone()).unwrap();
    let repair = wallet.client.full_scan(&restored).unwrap().unwrap();
    restored.apply_update(repair).unwrap();
    assert_eq!(restored.tip().timestamp(), Some(confirmed_tip.time));

    // A replaced timestamp and a same-height tip replacement both require an update.
    for replace_hash in [false, true] {
        let mut stale = snapshot.clone();
        stale.wollet_status = restored.status();
        stale.new_txs = Default::default();
        stale.txid_height_new.clear();
        stale.scripts_with_blinding_pubkey.clear();
        stale.timestamps = vec![(confirmed_tip.height, confirmed_tip.time - 1)];
        if replace_hash {
            stale.tip.time -= 1;
        }
        restored.apply_update(stale).unwrap();
        let repair = wallet.client.full_scan(&restored.state()).unwrap().unwrap();
        restored.apply_update(repair).unwrap();
        assert_eq!(restored.tip().hash(), confirmed_tip.block_hash());
        assert_eq!(restored.tip().timestamp(), Some(confirmed_tip.time));
        assert!(wallet.client.full_scan(&restored).unwrap().is_none());
    }

    // A known transaction reappearing needs no download but must be restored.
    snapshot.wollet_status = restored.status();
    snapshot.new_txs = Default::default();
    snapshot.txid_height_new.clear();
    snapshot.txid_height_delete = vec![txid];
    snapshot.scripts_with_blinding_pubkey.clear();
    snapshot.last_unused.external = 3;
    restored.apply_update(snapshot).unwrap();
    let update = wallet.client.full_scan(&restored).unwrap().unwrap();
    assert!(update.new_txs.txs.is_empty());
    restored.apply_update(update).unwrap();
    assert!(restored.transaction(&txid).unwrap().is_some());
    assert_eq!(restored.address(None).unwrap().index(), 3);
    assert!(wallet.client.full_scan(&restored).unwrap().is_none());

    // A transaction below the tip must not cause repeated timestamp-only updates.
    env.elementsd_generate(1);
    for _ in 0..100 {
        if wallet.client.tip().unwrap().height > wallet.wollet.tip().height() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let update = wallet.client.full_scan(&wallet.wollet).unwrap().unwrap();
    assert!(update.only_tip());
    wallet.wollet.apply_update(update).unwrap();
    assert!(wallet.client.full_scan(&wallet.wollet).unwrap().is_none());
    assert!(wallet
        .client
        .full_scan(&wallet.wollet.state())
        .unwrap()
        .is_none());

    // Restoring persisted updates must preserve the same no-op decision.
    wallet.wollet =
        WolletBuilder::new(Network::default_regtest(), TEST_DESCRIPTOR.parse().unwrap())
            .with_legacy_fs_store(wallet.path())
            .unwrap()
            .build()
            .unwrap();
    assert!(wallet.client.full_scan(&wallet.wollet).unwrap().is_none());
}
