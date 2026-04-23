use crate::test_wollet::*;
use lwk_common::*;
use lwk_test_util::*;
use lwk_wollet::*;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_stores() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let network = ElementsNetwork::default_regtest();
    let lbtc = network.policy_asset();

    let dir = TempDir::new().unwrap();
    let store = Arc::new(FileStore::new(dir.path().to_path_buf()).unwrap());

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let wd: WolletDescriptor = d.parse().unwrap();

    // `with_stores` set some fields, function setting them must be called after `with_stores`
    let expected = "`with_stores` must be called before other functions";
    let err = WolletBuilder::new(network, wd.clone())
        .with_txs_store(store.clone())
        .with_stores(store.clone())
        .unwrap_err()
        .to_string();
    assert!(err.contains(expected));

    let err = WolletBuilder::new(network, wd.clone())
        .with_updates_store(store.clone())
        .with_stores(store.clone())
        .unwrap_err()
        .to_string();
    assert!(err.contains(expected));

    let err = WolletBuilder::new(network, wd.clone())
        .with_merge_threshold(Some(1))
        .with_stores(store.clone())
        .unwrap_err()
        .to_string();
    assert!(err.contains(expected));

    let err = WolletBuilder::new(network, wd.clone())
        .with_legacy_fs_store(&dir)
        .unwrap()
        .with_stores(store.clone())
        .unwrap_err()
        .to_string();
    assert!(err.contains(expected));

    let mut wollet = WolletBuilder::new(network, wd.clone())
        .with_stores(store.clone())
        .unwrap()
        .with_updates_store(store.clone())
        .with_txs_store(store.clone())
        .with_merge_threshold(Some(1))
        .build()
        .unwrap();
    let mut client = test_client_electrum(&env.electrum_url());

    let address = wollet.address(None).unwrap();
    let satoshi = 10_000;
    let txid = env.elementsd_sendtoaddress(address.address(), satoshi, Some(lbtc));
    wait_for_tx(&mut wollet, &mut client, &txid);

    // Check updates
    assert!(lwk_common::Store::get(store.as_ref(), "000000000000")
        .unwrap()
        .is_some());
    // Check txs
    let key_bytes = wd.encryption_key_bytes();
    let enc_store =
        EncryptedStore::new_with_key_encryption(store.clone() as Arc<dyn DynStore>, key_bytes);
    assert!(lwk_common::Store::get(&enc_store, "wollet:txids")
        .unwrap()
        .is_some());
}

#[test]
fn fake_txs_store_full_scan_after_transaction() {
    // we want to test a full scan with a FakeStore (which don't really store tx) works,
    // because tx_as_fallback fn use in-memory tx preferably
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let network = ElementsNetwork::default_regtest();
    let lbtc = network.policy_asset();

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let wd: WolletDescriptor = d.parse().unwrap();

    let mut wollet = WolletBuilder::new(network, wd)
        .with_txs_store(Arc::new(FakeStore::new()))
        .build()
        .unwrap();
    let mut client = test_client_electrum(&env.electrum_url());

    let address = wollet.address(None).unwrap();
    assert_eq!(address.index(), 0);

    let satoshi = 10_000;
    let txid = env.elementsd_sendtoaddress(address.address(), satoshi, Some(lbtc));

    // do apply_update_internally
    wait_for_tx(&mut wollet, &mut client, &txid);

    assert_eq!(wollet.address(None).unwrap().index(), 1);
    assert_eq!(wollet.txs(&TxsOpt::without_tx()).unwrap().len(), 1);
}
