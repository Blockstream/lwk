use crate::test_wollet::*;
use elements::Txid;
use lwk_common::*;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::*;
use std::collections::HashSet;
use std::sync::Arc;
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
    let mut wollet = WolletBuilder::new(network, wd.clone())
        .with_store(store.clone())
        .with_txs_store(store.clone(), encrypt_txs_store)
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
    assert!(persisted_txids.contains(&txid1));
    assert!(persisted_txids.contains(&txid2));

    // check that a new wollet using the same store has all the txs (without syncing)
    let wollet2 = WolletBuilder::new(network, wd.clone())
        .with_store(store.clone())
        .with_txs_store(store.clone(), encrypt_txs_store)
        .build()
        .unwrap();

    let opt = TxsOpt::default();
    let txs = wollet2.txs(&opt).unwrap();
    assert_eq!(txs.len(), 2);
    assert!(txs.iter().any(|tx| tx.txid() == txid1));
    assert!(txs.iter().any(|tx| tx.txid() == txid2));
}
