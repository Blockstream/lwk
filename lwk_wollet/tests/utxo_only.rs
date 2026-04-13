use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use lwk_common::Signer;
use lwk_signer::*;
use lwk_test_util::*;
use lwk_wollet::clients::EsploraClientBuilder;
use lwk_wollet::*;
use std::str::FromStr;

#[test]
fn test_esplora_waterfalls_utxo_only() {
    let env = TestEnvBuilder::from_env().with_waterfalls().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let desc = WolletDescriptor::from_str(&desc).unwrap();

    let network = ElementsNetwork::default_regtest();
    let mut wollet = WolletBuilder::new(network, desc.clone()).build().unwrap();
    let mut client = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .build_blocking()
        .unwrap();

    let mut wollet_utxo_only = WolletBuilder::new(network, desc.clone())
        .utxo_only(true)
        .build()
        .unwrap();
    let mut client_utxo_only = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .utxo_only(true)
        .build_blocking()
        .unwrap();

    let address = wollet.address(None).unwrap();
    let _txid = env.elementsd_sendtoaddress(address.address(), 1_000_000, None);
    std::thread::sleep(std::time::Duration::from_millis(2_000));

    // check both wallets have the same balance
    let update = client.full_scan(&wollet).unwrap().unwrap();
    wollet.apply_update(update).unwrap();
    let update = client_utxo_only
        .full_scan(&wollet_utxo_only)
        .unwrap()
        .unwrap();
    wollet_utxo_only.apply_update(update).unwrap();
    assert_eq!(
        format!("{:?}", *wollet.balance().unwrap()),
        "{5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225: 1000000}"
    );
    assert_eq!(
        wollet.balance().unwrap(),
        wollet_utxo_only.balance().unwrap()
    );
    assert_eq!(wollet.utxos().unwrap().len(), 1);
    assert_eq!(wollet_utxo_only.utxos().unwrap().len(), 1);
    assert_eq!(
        wollet.transactions().unwrap(),
        wollet_utxo_only.transactions().unwrap()
    );

    // spend from wollet and sync again both wallets
    let address = env.elementsd_getnewaddress();
    let mut pset = wollet
        .tx_builder()
        .add_lbtc_recipient(&address, 100_000)
        .unwrap()
        .finish()
        .unwrap();
    signer.sign(&mut pset).unwrap();
    let pset_details = wollet.get_details(&pset).unwrap();

    let tx = wollet.finalize(&mut pset).unwrap();

    client.broadcast(&tx).unwrap();

    env.elementsd_generate(1); // TODO: remove this
    std::thread::sleep(std::time::Duration::from_millis(2_000));

    let update = client.full_scan(&wollet).unwrap().unwrap();
    wollet.apply_update(update).unwrap();
    let update = client_utxo_only
        .full_scan(&wollet_utxo_only)
        .unwrap()
        .unwrap();
    wollet_utxo_only.apply_update(update).unwrap();

    assert_eq!(
        format!("{:?}", *wollet.balance().unwrap()),
        format!(
            "{{5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225: {}}}",
            1_000_000 - pset_details.balance.fee - 100_000
        )
    );
    assert_eq!(
        wollet.balance().unwrap(),
        wollet_utxo_only.balance().unwrap()
    );

    assert_eq!(wollet.utxos().unwrap().len(), 1);
    assert_eq!(wollet_utxo_only.utxos().unwrap().len(), 1);

    assert_eq!(wollet_utxo_only.transactions().unwrap().len(), 1);
    assert_eq!(wollet.transactions().unwrap().len(), 2);

    // ensure the dummy tx is not in the transactions list, the dummy_tx has zero outputs.
    assert!(wollet_utxo_only
        .transactions()
        .unwrap()
        .iter()
        .all(|tx| !tx.outputs.is_empty()));
}

#[test]
fn test_waterfalls_utxo_only_with_dummy() {
    let env = TestEnvBuilder::from_env().with_waterfalls().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/<0;1>/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];
    let network = ElementsNetwork::default_regtest();
    let lbtc = network.policy_asset();

    let client = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .build_blocking()
        .unwrap();
    let mut w = TestWollet::new(client, &desc);

    let wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut client_utxo_only = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .utxo_only(true)
        .build_blocking()
        .unwrap();
    let mut wollet_utxo_only = WolletBuilder::new(network, wd)
        .utxo_only(true)
        .build()
        .unwrap();

    let node_address = env.elementsd_getnewaddress();
    let txid0 = w.fund_btc(&env);
    let txid1 = w.send_btc(&signers, None, Some((node_address.clone(), 1)));
    env.elementsd_generate(1);

    // Utxo only intermediate sync
    let update = client_utxo_only
        .full_scan(&wollet_utxo_only)
        .unwrap()
        .unwrap();
    // No more dummy txs
    assert!(!update
        .new_txs
        .txs
        .iter()
        .any(|(_, tx)| tx.output.is_empty()));
    wollet_utxo_only.apply_update(update).unwrap();

    let txid2 = w.send_btc(&signers, None, Some((node_address.clone(), 1)));
    let txid3 = w.send_btc(&signers, None, Some((node_address.clone(), 1)));
    env.elementsd_generate(1);
    wait_for_tx_confirmation(&mut w.wollet, &mut w.client, &txid3);

    let balance = w.wollet.balance().unwrap();
    assert!(*balance.get(&lbtc).unwrap_or(&0) > 0);
    let txs = w.wollet.transactions().unwrap();
    assert_eq!(txs.len(), 4);
    assert!(txs.iter().any(|tx| tx.txid == txid0));
    assert!(txs.iter().any(|tx| tx.txid == txid1));
    assert!(txs.iter().any(|tx| tx.txid == txid2));
    assert!(txs.iter().any(|tx| tx.txid == txid3));

    // Utxo only final sync
    let update = client_utxo_only
        .full_scan(&wollet_utxo_only)
        .unwrap()
        .unwrap();
    // No more dummy txs
    assert!(!update
        .new_txs
        .txs
        .iter()
        .any(|(_, tx)| tx.output.is_empty()));
    wollet_utxo_only.apply_update(update).unwrap();

    assert_eq!(wollet_utxo_only.balance().unwrap(), balance);
    let txs = wollet_utxo_only.transactions().unwrap();
    assert_eq!(txs.len(), 1);
    assert!(txs.iter().any(|tx| tx.txid == txid3));
}

async fn test_esplora_waterfalls_balance_comparison(
    descriptor: &str,
    esplora_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let desc = WolletDescriptor::from_str(descriptor)?;
    let network = ElementsNetwork::LiquidTestnet;

    let mut wollet = WolletBuilder::new(network, desc.clone()).build()?;
    let mut client = EsploraClientBuilder::new(esplora_url, network)
        .waterfalls(true)
        .concurrency(4)
        .build()?;

    let mut wollet_utxo_only = WolletBuilder::new(network, desc.clone())
        .utxo_only(true)
        .build()?;
    let mut client_utxo_only = EsploraClientBuilder::new(esplora_url, network)
        .utxo_only(true)
        .waterfalls(true)
        .concurrency(4)
        .build()?;

    // Perform full scan on both wallets
    let update = client.full_scan(&wollet).await?.unwrap();
    wollet.apply_update(update)?;

    let update = client_utxo_only
        .full_scan(&wollet_utxo_only)
        .await?
        .unwrap();
    wollet_utxo_only.apply_update(update)?;

    let u1 = wollet.utxos()?;
    let u2 = wollet_utxo_only.utxos()?;
    assert_eq!(u1.len(), u2.len());
    assert_eq!(u1, u2);

    // Compare balances
    let balance = wollet.balance()?;
    let balance_utxo_only = wollet_utxo_only.balance()?;

    assert_eq!(balance, balance_utxo_only);

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_esplora_waterfalls_testnet_utxo_only_1() {
    let descriptor = "ct(slip77(4892ff8181d55103c9b0a3a0ec2eb384a7518c51a87d59a9da011ce671d6e657),elwpkh([8fd75c12/84'/1'/0']tpubDDkuNJ5AvNAgekVh7Y4sAkmCzKs7mySbuq1GSnpA3oM7XxkCWVnT7y8ZSbbHFYxQYkdxNdzinLKt6kBKSVYD75UEHduiVjNz24Ew8YgpS5E/<0;1>/*))#qfvkjcee";
    let esplora_url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";

    test_esplora_waterfalls_balance_comparison(descriptor, esplora_url)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_esplora_waterfalls_testnet_utxo_only_2() {
    let descriptor = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
    let esplora_url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";

    test_esplora_waterfalls_balance_comparison(descriptor, esplora_url)
        .await
        .unwrap();
}

#[test]
fn test_faucet() {
    // Simulate a couple of errors that we see with the testnet faucet
    let env = TestEnvBuilder::from_env().with_waterfalls().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let client = EsploraClientBuilder::new(&env.waterfalls_url(), ElementsNetwork::Liquid)
        .utxo_only(true)
        .waterfalls(true)
        .build_blocking()
        .unwrap();
    let opt = TestWolletOpt { utxo_only: true };
    let mut w = TestWollet::with_opt(client, &desc, &opt);

    let lbtc = w.policy_asset();
    let txid0 = w.fund_btc(&env);

    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].outpoint.txid, txid0);

    let node_address = env.elementsd_getnewaddress();
    let mut pset = w
        .tx_builder()
        .add_recipient(&node_address, 1000, lbtc)
        .unwrap()
        .finish()
        .unwrap();
    let sigs = signer.sign(&mut pset).unwrap();
    assert!(sigs > 0);
    let tx = w.wollet.finalize(&mut pset).unwrap();

    let txid1 = tx.txid();
    w.wollet.apply_transaction(tx.clone()).unwrap();

    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].outpoint.txid, txid1);

    // Simulate a situation where the tx is seen by the node *after* the next sync
    w.sync();
    w.client.broadcast(&tx).unwrap();

    // We see the old state
    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].outpoint.txid, txid0);

    // Sending a new tx will trigger an error
    let mut pset = w
        .tx_builder()
        .add_recipient(&node_address, 1000, lbtc)
        .unwrap()
        .finish()
        .unwrap();
    let sigs = signer.sign(&mut pset).unwrap();
    assert!(sigs > 0);
    let tx = w.wollet.finalize(&mut pset).unwrap();
    let err = w.client.broadcast(&tx).unwrap_err();
    assert!(err.to_string().contains("txn-mempool-conflict"));

    // If the tx is included in a block, the error is different
    env.elementsd_generate(1);
    let err = w.client.broadcast(&tx).unwrap_err();
    assert!(err.to_string().contains("bad-txns-inputs-missingorspent"));
}

#[test]
fn test_incompatible_utxo_only() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .with_waterfalls()
        .build();

    let network = ElementsNetwork::default_regtest();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();

    let wollet = WolletBuilder::new(network, wd.clone()).build().unwrap();
    assert!(!wollet.utxo_only());
    let wollet = WolletBuilder::new(network, wd.clone())
        .utxo_only(false)
        .build()
        .unwrap();
    assert!(!wollet.utxo_only());
    let wollet_uo = WolletBuilder::new(network, wd.clone())
        .utxo_only(true)
        .build()
        .unwrap();
    assert!(wollet_uo.utxo_only());

    let mut client_electrum = test_client_electrum(&env.electrum_url());
    let mut client_esplora = EsploraClientBuilder::new(&env.esplora_url(), network)
        .build_blocking()
        .unwrap();
    let mut client_waterfalls = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .build_blocking()
        .unwrap();
    let mut client_waterfalls_uo = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .waterfalls(true)
        .utxo_only(true)
        .build_blocking()
        .unwrap();

    let err = client_electrum.full_scan(&wollet_uo).unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
    let err = client_esplora.full_scan(&wollet_uo).unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
    let err = client_waterfalls.full_scan(&wollet_uo).unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
    let err = client_waterfalls_uo.full_scan(&wollet).unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));

    // TODO: consider making apply_update fail too
    // (requires Update to store whether it was utxo_only)
}

#[tokio::test]
async fn test_incompatible_utxo_only_async() {
    // Note: this test does 0 requests, so any value works here
    let waterfalls_url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    let esplora_url = "https://blockstream.info/liquidtestnet/api";

    let network = ElementsNetwork::LiquidTestnet;
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();

    let wollet = WolletBuilder::new(network, wd.clone()).build().unwrap();
    let wollet_uo = WolletBuilder::new(network, wd.clone())
        .utxo_only(true)
        .build()
        .unwrap();

    let mut client_esplora = EsploraClientBuilder::new(esplora_url, network)
        .build()
        .unwrap();
    let mut client_waterfalls = EsploraClientBuilder::new(waterfalls_url, network)
        .waterfalls(true)
        .build()
        .unwrap();
    let mut client_waterfalls_uo = EsploraClientBuilder::new(waterfalls_url, network)
        .waterfalls(true)
        .utxo_only(true)
        .build()
        .unwrap();

    let err = client_esplora.full_scan(&wollet_uo).await.unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
    let err = client_waterfalls.full_scan(&wollet_uo).await.unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
    let err = client_waterfalls_uo.full_scan(&wollet).await.unwrap_err();
    assert!(matches!(err, Error::UtxoOnlyIncompatible));
}

#[test]
fn test_waterfalls_utxo_only_persisted() {
    let env = TestEnvBuilder::from_env().with_waterfalls().build();
    let network = ElementsNetwork::default_regtest();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let client = EsploraClientBuilder::new(&env.waterfalls_url(), network)
        .utxo_only(true)
        .waterfalls(true)
        .build_blocking()
        .unwrap();
    let opt = TestWolletOpt { utxo_only: true };
    let mut w = TestWollet::with_opt(client, &desc, &opt);

    let lbtc = w.policy_asset();
    let _txid0 = w.fund_btc(&env);
    let updates = w.wollet.updates().unwrap();
    assert!(!updates.is_empty());
    assert!(updates.iter().any(|u| !u.unspent.is_empty()));
    let balance = w.balance(&lbtc);

    // restart using the same datadir
    let wollet = WolletBuilder::new(network, desc.parse().unwrap())
        .with_legacy_fs_store(w.path())
        .unwrap()
        .utxo_only(true)
        .build()
        .unwrap();
    // From persisted updates
    let updates = wollet.updates().unwrap();
    assert!(!updates.is_empty());
    assert!(updates.iter().any(|u| !u.unspent.is_empty()));
    let new_balance = wollet.balance().unwrap();
    assert_eq!(new_balance.get(&lbtc).unwrap(), &balance);
}
