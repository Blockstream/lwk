use crate::test_wollet::*;
use elements::hex::ToHex;
use lwk_common::Signer;
use lwk_signer::bip39::Mnemonic;
use lwk_signer::SwSigner;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::clients::blocking::EsploraClient;
use lwk_wollet::{Network, WolletBuilder, WolletDescriptor};
use rand::{thread_rng, Rng};
use std::str::FromStr;

#[test]
fn test_single_address_tr() {
    // Monitor a wallet that consists in a single taproot address
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let view_key = generate_view_key();
    let pk = "0202020202020202020202020202020202020202020202020202020202020202";

    let desc = format!("ct({view_key},eltr({pk}))");
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    w.fund_btc(&env);
    let balance = w.balance_btc();
    assert!(balance > 0);
    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);

    // Receive unconfidential / explicit
    let satoshi = 5_000;
    w.fund_explicit(&env, satoshi, None, None);
    assert_eq!(w.balance_btc(), balance);

    let explicit_utxos = w.wollet.explicit_utxos().unwrap();
    assert_eq!(explicit_utxos.len(), 1);

    // Signing is not supported
    let mut pset = w
        .tx_builder()
        .add_lbtc_recipient(&w.address(), 1000)
        .unwrap()
        .finish()
        .unwrap();

    let signer = generate_signer();
    let sigs_added = signer.sign(&mut pset).unwrap();
    assert_eq!(sigs_added, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_address_tr_async() {
    let env = TestEnvBuilder::from_env().with_esplora().build();

    let view_key = generate_view_key();
    let pk = "0202020202020202020202020202020202020202020202020202020202020202";

    let desc = format!("ct({view_key},eltr({pk}))");
    let mut client =
        lwk_wollet::asyncr::EsploraClient::new(Network::default_regtest(), &env.esplora_url());
    let network = Network::default_regtest();
    let descriptor: WolletDescriptor = desc.parse().unwrap();
    let mut wollet = WolletBuilder::new(network, descriptor).build().unwrap();

    let addr = wollet.address(None).unwrap();

    let _txid = env.elementsd_sendtoaddress(addr.address(), 2_000_011, None);

    // TODO: wait_update_with_txs is not working correctly in this case. It seems
    // it returns even if the tx is not yet in the wallet, fix it and remove this unconditional wait
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let update = client.full_scan(&wollet).await.unwrap().unwrap();

    wollet.apply_update(update).unwrap();

    let balance = *wollet
        .balance()
        .unwrap()
        .get(&regtest_policy_asset())
        .unwrap();
    assert!(balance > 0);
    let utxos = wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 1);
}

#[test]
fn test_single_tr_sign() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnvBuilder::from_env().with_esplora().build();

    let is_mainnet = false;
    let bip = lwk_common::Bip::Bip86;
    let network = env.elementsd_network();
    let lbtc = *network.policy_asset();

    // Alice creates their signer and gets the xpub
    let mnemonic_a = Mnemonic::generate(12)?;
    let signer_a = SwSigner::new(&mnemonic_a.to_string(), is_mainnet)?.with_network(network);
    let xpub_a = dbg!(signer_a.keyorigin_xpub(bip, is_mainnet)?);

    // Bob creates their signer and gets the xpub
    let mnemonic_b = Mnemonic::generate(12)?;
    let signer_b = SwSigner::new(&mnemonic_b.to_string(), is_mainnet)?.with_network(network);
    let xpub_b = signer_b.keyorigin_xpub(bip, is_mainnet)?;

    // Generate a random SLIP77 descriptor blinding key
    let mut slip77_rand_key = [0u8; 32];
    thread_rng().fill(&mut slip77_rand_key);
    let slip77_rand_key = slip77_rand_key.to_hex();
    let desc_blinding_key = format!("slip77({slip77_rand_key})");

    let desc1 = dbg!(format!("ct({desc_blinding_key},eltr({xpub_a}/<0;1>/*))"));
    let desc2 = dbg!(format!("ct({desc_blinding_key},eltr({xpub_b}/<0;1>/*))"));

    let wd1 = WolletDescriptor::from_str(&desc1)?;
    let wd2 = WolletDescriptor::from_str(&desc2)?;

    let mut wallet_a = WolletBuilder::new(network, wd1).build()?;
    let mut wallet_b = WolletBuilder::new(network, wd2).build()?;

    let addr_a = wallet_a.address(None)?;
    let addr_b = wallet_b.address(None)?;

    let url = env.esplora_url();
    let mut client = EsploraClient::new(&url, network)?;

    if let Some(update) = client.full_scan(&wallet_a)? {
        wallet_a.apply_update(update)?;
    }
    if let Some(update) = client.full_scan(&wallet_b)? {
        wallet_b.apply_update(update)?;
    }

    // Receive some funds
    let wallet_a_replenish = 10_000;
    let txid = env.elementsd_sendtoaddress(addr_a.address(), wallet_a_replenish, None);
    wait_for_tx(&mut wallet_a, &mut client, &txid);

    assert_eq!(
        *wallet_a
            .balance()?
            .get(env.elementsd_network().policy_asset())
            .unwrap(),
        10_000
    );

    // Alice sends `sats` to Bob
    let sats = 3000;

    if let Some(update) = client.full_scan(&wallet_a)? {
        wallet_a.apply_update(update)?;
    }
    if let Some(update) = client.full_scan(&wallet_b)? {
        wallet_b.apply_update(update)?;
    }

    let mut pset = wallet_a
        .tx_builder()
        .add_recipient(addr_b.address(), sats, lbtc)?
        .finish()?;

    let sigs_added = dbg!(signer_b.sign(&mut pset)?);
    assert_eq!(sigs_added, 0);
    let sigs_added = dbg!(signer_a.sign(&mut pset)?);
    assert_eq!(sigs_added, 1);

    let tx = wallet_a.finalize(&mut pset)?;
    let txid = client.broadcast(&tx)?;
    let _ = client.get_transaction(txid)?;

    if let Some(update) = client.full_scan(&wallet_b)? {
        wallet_b.apply_update(update)?;
    }
    assert_eq!(
        *wallet_b
            .balance()?
            .get(env.elementsd_network().policy_asset())
            .unwrap(),
        sats
    );

    // Bob sends `sats` to arbitrary user
    let sats = 2000;
    let address = env.elementsd_getnewaddress();

    if let Some(update) = client.full_scan(&wallet_a)? {
        wallet_a.apply_update(update)?;
    }
    if let Some(update) = client.full_scan(&wallet_b)? {
        wallet_b.apply_update(update)?;
    }

    let mut pset = wallet_b
        .tx_builder()
        .add_recipient(&address, sats, lbtc)?
        .finish()?;

    let sigs_added = dbg!(signer_a.sign(&mut pset)?);
    assert_eq!(sigs_added, 0);
    let sigs_added = dbg!(signer_b.sign(&mut pset)?);
    assert_eq!(sigs_added, 1);

    let tx = wallet_a.finalize(&mut pset)?;
    let txid = client.broadcast(&tx)?;
    let _ = client.get_transaction(txid)?;

    Ok(())
}

#[test]
fn test_tr_sign_multiple_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnvBuilder::from_env().with_esplora().build();

    let is_mainnet = false;
    let bip = lwk_common::Bip::Bip86;
    let network = env.elementsd_network();
    let lbtc = *network.policy_asset();

    // Alice creates their signer and gets the xpub
    let mnemonic = Mnemonic::generate(12)?;
    let signer = SwSigner::new(&mnemonic.to_string(), is_mainnet)?.with_network(network);
    let xpub = signer.keyorigin_xpub(bip, is_mainnet)?;

    // Generate a random SLIP77 descriptor blinding key
    let mut slip77_rand_key = [0u8; 32];
    thread_rng().fill(&mut slip77_rand_key);
    let desc_blinding_key = format!("slip77({})", slip77_rand_key.to_hex());

    let desc = format!("ct({desc_blinding_key},eltr({xpub}/<0;1>/*))");
    let wd = WolletDescriptor::from_str(&desc)?;
    let mut wallet = WolletBuilder::new(network, wd).build()?;
    let addr = wallet.address(None)?;

    // Send two separate transactions to create 2 inputs
    let txid1 = env.elementsd_sendtoaddress(addr.address(), 6000, None);
    let addr2 = wallet.address(None)?;
    let txid2 = env.elementsd_sendtoaddress(addr2.address(), 7000, None);

    let url = env.esplora_url();
    let mut esplora_client = EsploraClient::new(&url, network)?;

    wait_for_tx(&mut wallet, &mut esplora_client, &txid1);
    wait_for_tx(&mut wallet, &mut esplora_client, &txid2);

    if let Some(update) = esplora_client.full_scan(&wallet)? {
        wallet.apply_update(update)?;
    }

    assert_eq!(wallet.utxos()?.len(), 2);

    let address = env.elementsd_getnewaddress();
    let mut pset = wallet
        .tx_builder()
        .add_recipient(&address, 10000, lbtc)?
        .finish()?;

    // The transaction should consume both inputs
    assert_eq!(pset.inputs().len(), 2);

    // Check that tap_internal_key is correctly set in inputs
    for input in pset.inputs() {
        assert!(input.tap_internal_key.is_some());
    }

    let sigs_added = signer.sign(&mut pset)?;
    assert_eq!(sigs_added, 2);

    let tx = wallet.finalize(&mut pset)?;
    let txid = esplora_client.broadcast(&tx)?;
    let _ = esplora_client.get_transaction(txid)?;

    Ok(())
}
