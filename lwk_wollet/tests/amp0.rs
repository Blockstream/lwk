use crate::test_wollet::*;
use lwk_common::Signer;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::BlockchainBackend;

#[test]
fn test_blinding_nonces() {
    // Construct a transaction and obtain the blinding nonces
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    let lbtc = w.policy_asset();
    w.fund_btc(&env);

    let node_addr = env.elementsd_getnewaddress();
    let amp0pset = w
        .tx_builder()
        .add_recipient(&node_addr, 1000, lbtc)
        .unwrap()
        .finish_for_amp0()
        .unwrap();
    let mut pset = amp0pset.pset().clone();
    let blinding_nonces = amp0pset.blinding_nonces();

    let sigs = signer.sign(&mut pset).unwrap();
    assert!(sigs > 0);

    w.send(&mut pset);

    // Amp0Pset::new checks that blinding nonces and PSET are consistent
    let fake_blinding_nonces = vec![String::new(); blinding_nonces.len()];
    let res = lwk_wollet::amp0::Amp0Pset::new(pset, fake_blinding_nonces);
    assert!(res.is_err());
}

#[test]
#[allow(unused)]
#[cfg(feature = "amp0")]
#[ignore = "requires network calls"]
fn test_amp0_setup() -> Result<(), Box<dyn std::error::Error>> {
    // ANCHOR: amp0-setup
    use lwk_common::{Amp0Signer, Network};
    use lwk_signer::SwSigner;
    use lwk_wollet::amp0::blocking::{Amp0, Amp0Connected};

    // Create signer and watch only credentials
    let network = Network::TestnetLiquid;
    let is_mainnet = false;
    let (signer, mnemonic) = SwSigner::random(is_mainnet)?;
    let username = "<username>";
    let password = "<password>";
    let username = format!("user{}", signer.fingerprint()); // ANCHOR: ignore
    let password = format!("pass{}", signer.fingerprint()); // ANCHOR: ignore

    // Collect signer data
    let signer_data = signer.amp0_signer_data()?;

    // Connect to AMP0
    let amp0 = Amp0Connected::new(network, signer_data)?;

    // Obtain and sign the authentication challenge
    let challenge = amp0.get_challenge()?;
    let sig = signer.amp0_sign_challenge(&challenge)?;

    // Login
    let mut amp0 = amp0.login(&sig)?;

    // Create a new AMP0 account
    let pointer = amp0.next_account()?;
    let account_xpub = signer.amp0_account_xpub(pointer)?;
    let amp_id = amp0.create_amp0_account(pointer, &account_xpub)?;

    // Create watch only entries
    amp0.create_watch_only(&username, &password)?;

    // Use watch only credentials to interact with AMP0
    let amp0 = Amp0::new(network, &username, &password, &amp_id)?;
    // ANCHOR_END: amp0-setup

    Ok(())
}

#[test]
#[allow(unused)]
#[cfg(feature = "amp0")]
#[ignore = "requires network calls"]
#[rustfmt::skip] // our priority here is that generated docs renders nicely
fn test_amp0_daily_ops() -> Result<(), Box<dyn std::error::Error>> {
    // ANCHOR: amp0-daily-ops
    use lwk_common::{Network, Signer};
    use lwk_signer::SwSigner;
    use lwk_wollet::amp0::{blocking::Amp0, Amp0Pset};
    use lwk_wollet::{clients::blocking::EsploraClient, ElementsNetwork, Wollet};

    // Signer
    let mnemonic = "<mnemonic>";
    // AMP0 Watch-Only credentials
    let username = "<username>";
    let password = "<password>";
    let mnemonic = "thrive metal cactus come oval candy medal bounce captain shock permit joke"; // ANCHOR: ignore
    let username = "userlwk001"; // ANCHOR: ignore
    let password = "userlwk001"; // ANCHOR: ignore
    // AMP ID (optional)
    let amp_id = "";

    // Create AMP0 context
    let network = Network::TestnetLiquid;

    let mut amp0 = Amp0::new(network, username, password, amp_id)?;

    // Create AMP0 Wollet
    let wd = amp0.wollet_descriptor();
    let mut wollet = Wollet::without_persist(ElementsNetwork::LiquidTestnet, wd)?;

    // Get a new address
    let addr = amp0.address(None);

    // Update the wallet with (new) blockchain data
    let url = "https://blockstream.info/liquidtestnet/api";
    let mut client = EsploraClient::new(url, ElementsNetwork::LiquidTestnet)?;
    // esplora is too slow // ANCHOR: ignore
    let url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api"; // ANCHOR: ignore
    let mut client = EsploraClient::new_waterfalls(url, ElementsNetwork::LiquidTestnet)?; // ANCHOR: ignore
    if let Some(update) = client.full_scan_to_index(&wollet, amp0.last_index())? {
        wollet.apply_update(update)?;
    }

    // Get balance
    let balance = wollet.balance()?;
    let lbtc = wollet.policy_asset(); // ANCHOR: ignore
    let balance = *balance.get(&lbtc).unwrap_or(&0); // ANCHOR: ignore
    if balance < 500 { // ANCHOR: ignore
        let addr = amp0.address(Some(1)).unwrap(); // ANCHOR: ignore
        panic!("Send some tLBTC to {}", addr.address()); // ANCHOR: ignore
    } // ANCHOR: ignore

    // Construct a PSET sending LBTC back to the wallet
    let amp0pset = wollet
        .tx_builder()
        .drain_lbtc_wallet()
        .finish_for_amp0()?;
    let mut pset = amp0pset.pset().clone();
    let blinding_nonces = amp0pset.blinding_nonces();

    // User signs the PSET
    let is_mainnet = false;
    let signer = SwSigner::new(mnemonic, is_mainnet)?;
    let sigs = signer.sign(&mut pset)?;
    assert!(sigs > 0);

    // Reconstruct the Amp0 PSET with the PSET signed by the user
    let amp0pset = Amp0Pset::new(pset, blinding_nonces.to_vec())?;

    // AMP0 signs
    let tx = amp0.sign(&amp0pset)?;

    // Broadcast the transaction
    let txid = client.broadcast(&tx)?;
    // ANCHOR_END: amp0-daily-ops

    Ok(())
}
