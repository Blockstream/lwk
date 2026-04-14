use crate::test_wollet::*;
use elements::Txid;
use lwk_common::*;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_txs_store() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let network = ElementsNetwork::default_regtest();
    let lbtc = network.policy_asset();

    let dir = TempDir::new().unwrap();
    let store = Arc::new(FileStore::new(dir.path().to_path_buf()).unwrap());

    let s = generate_signer();
    let view_key = generate_view_key();
    let d = format!("ct({view_key},elwpkh({}/*))", s.xpub());
    let wd: WolletDescriptor = d.parse().unwrap();
    // We use an unencrypted file store, let's follow the the doc suggestion and make the wollet encrypting it
    let encrypt_txs_store = true;

    let err = WolletBuilder::new(network, wd.clone())
        .with_txs_store(store.clone(), encrypt_txs_store)
        .build()
        .unwrap_err()
        .to_string();
    let expected = "If txs store is persited, merge threshold must be 1";
    assert!(err.contains(expected));

    let mut wollet = WolletBuilder::new(network, wd.clone())
        .with_store(store.clone())
        .with_txs_store(store.clone(), encrypt_txs_store)
        .with_merge_threshold(Some(1))
        .build()
        .unwrap();
    let mut client = test_client_electrum(&env.electrum_url());

    let address = wollet.address(None).unwrap();
    let satoshi = 10_000;
    let txid1 = env.elementsd_sendtoaddress(address.address(), satoshi, Some(lbtc));
    wait_for_tx(&mut wollet, &mut client, &txid1);

    let address = env.elementsd_getnewaddress();
    let mut pset = wollet
        .tx_builder()
        .add_lbtc_recipient(&address, 1_000)
        .unwrap()
        .finish()
        .unwrap();
    let sigs = s.sign(&mut pset).unwrap();
    assert!(sigs > 0);
    let tx = wollet.finalize(&mut pset).unwrap();
    let txid2 = client.broadcast(&tx).unwrap();
    wait_for_tx(&mut wollet, &mut client, &txid2);

    // they are reachable with the encrypted wrapper
    let key_bytes = wd.encryption_key_bytes();
    let enc_store =
        EncryptedStore::new_with_key_encryption(store.clone() as Arc<dyn DynStore>, key_bytes);
    let all_txids: HashSet<Txid> = lwk_common::Store::get(&enc_store, "wollet:txids")
        .unwrap()
        .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    assert_eq!(all_txids.len(), 2);
    assert!(all_txids.contains(&txid1));
    assert!(all_txids.contains(&txid2));

    // check that the persisted updates in Wollet.store have the full txs in them
    let updates = wollet.updates().unwrap();
    let persisted_txids: HashSet<Txid> = updates
        .iter()
        .flat_map(|u| u.new_txs.txs.iter().map(|(txid, _)| *txid))
        .collect();
    assert!(!persisted_txids.contains(&txid1));
    assert!(!persisted_txids.contains(&txid2));

    // check that a new wollet using the same store has all the txs (without syncing)
    let wollet2 = WolletBuilder::new(network, wd.clone())
        .with_store(store.clone())
        .with_txs_store(store.clone(), encrypt_txs_store)
        .with_merge_threshold(Some(1))
        .build()
        .unwrap();

    let opt = TxsOpt::default();
    let txs = wollet2.txs(&opt).unwrap();
    assert_eq!(txs.len(), 2);
    assert!(txs.iter().any(|tx| tx.txid() == txid1));
    assert!(txs.iter().any(|tx| tx.txid() == txid2));

    // To ensure we have the same txids, we need to check the status
    assert_eq!(wollet.status(), wollet2.status());
}

#[ignore = "require network calls"]
#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_txs_store_huge() {
    // cargo test --release -p lwk_wollet txs_store_huge -- --nocapture --include-ignored

    // more than 6k txs
    let desc = "ct(slip77(1bda6cd71a1e206e3eb793e5a4d98a46c3fa473c9ab7bdef9bb9c814764d6614),elwpkh([cb4ba44a/84'/1'/0']tpubDDrybtUajFcgXC85rvwPsh1oU7Azx4kJ9BAiRzMbByqK7UnVXY3gDRJPwEDfaQwguNUZFzrhavJGgEhbsfuebyxUSZQnjLezWVm2Vdqb7UM/<0;1>/*))#za9ktavp";
    // normal -- here for debugging
    // let desc = "ct(e350a44c4dad493e7b1faf4ef6a96c1ad13a6fb8d03d61fcec561afb8c3bae18,elwpkh([a8874235/84'/1776'/0']xpub6DLHCiTPg67KE9ksCjNVpVHTRDHzhCSmoBTKzp2K4FxLQwQvvdNzuqxhK2f9gFVCN6Dori7j2JMLeDoB4VqswG7Et9tjqauAvbDmzF8NEPH/<0;1>/*))#3axrmm5c";
    let url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";

    let network = ElementsNetwork::LiquidTestnet;

    let txs_dir = TempDir::new().unwrap();
    let txs_store = Arc::new(FileStore::new(txs_dir.path().to_path_buf()).unwrap());
    let encrypt_txs_store = true;

    let upd_dir = TempDir::new().unwrap();
    let mut wollet = WolletBuilder::new(network, desc.parse().unwrap())
        .with_legacy_fs_store(&upd_dir)
        .unwrap()
        .with_txs_store(txs_store.clone(), encrypt_txs_store)
        .with_merge_threshold(Some(1))
        .build()
        .unwrap();
    let mut client = clients::asyncr::EsploraClientBuilder::new(url, network)
        .waterfalls(true)
        .concurrency(4)
        .build()
        .unwrap();

    let t0 = Instant::now();
    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    let t1 = Instant::now();
    let t = t1.duration_since(t0).as_millis();
    println!("1st full scan:    {:>6} ms", t);

    wollet.apply_update(update).unwrap();
    let t2 = Instant::now();
    let t = t2.duration_since(t1).as_millis();
    println!("1st apply update: {:>6} ms", t);

    // we apply any transaction to trigger an Update re-serialization
    let tx = elements::Transaction {
        version: 2,
        lock_time: elements::LockTime::ZERO,
        input: vec![],
        output: vec![],
    };
    wollet.apply_transaction(tx).unwrap();
    let t3 = Instant::now();
    let t = t3.duration_since(t2).as_millis();
    println!("apply tx:         {:>6} ms", t);

    let res = client.full_scan(&wollet).await.unwrap();
    let t4 = Instant::now();
    let t = t4.duration_since(t3).as_millis();
    println!("2nd full scan:    {:>6} ms", t);

    if let Some(update) = res {
        wollet.apply_update(update).unwrap();
    }
    let t5 = Instant::now();
    let t = t5.duration_since(t4).as_millis();
    println!("2nd apply update: {:>6} ms", t);

    let txs = wollet.txs(&TxsOpt::default()).unwrap();
    let t6 = Instant::now();
    let t = t6.duration_since(t5).as_millis();
    println!("get all txs:      {:>6} ms", t);
    println!("num txs: {:>6}", txs.len());

    // Restore wollet
    let wollet2 = WolletBuilder::new(network, desc.parse().unwrap())
        .with_legacy_fs_store(&upd_dir)
        .unwrap()
        .with_txs_store(txs_store.clone(), encrypt_txs_store)
        .with_merge_threshold(Some(1))
        .build()
        .unwrap();
    let t7 = Instant::now();
    let t = t7.duration_since(t6).as_millis();
    println!("restore wollet:   {:>6} ms", t);

    assert_eq!(wollet.status(), wollet2.status());
}
