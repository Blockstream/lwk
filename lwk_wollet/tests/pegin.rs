use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use elements::hex::{FromHex, ToHex};
use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};
use lwk_common::Signer;
use lwk_signer::SwSigner;
use lwk_test_util::*;
use lwk_wollet::pegin::{fetch_fed_peg, fetch_last_full_header};
use lwk_wollet::*;

#[test]
fn claim_pegin() {
    let env = TestEnvBuilder::from_env()
        .with_bitcoind()
        .with_waterfalls()
        .with_fedpeg_script(FED_PEG_SCRIPT)
        .build();
    let network = env.elementsd_network();
    let signer = SwSigner::new(TEST_MNEMONIC, false).unwrap();
    let descriptor: WolletDescriptor = format!(
        "ct(slip77({TEST_MNEMONIC_SLIP77}),elwpkh({}/*))",
        signer.xpub()
    )
    .parse()
    .unwrap();
    let mut wollet = WolletBuilder::new(network, descriptor).build().unwrap();
    let mut client =
        clients::blocking::WaterfallsClient::new(&env.waterfalls_url(), network).unwrap();
    let update = client.full_scan(&wollet).unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    let fed_peg = fetch_fed_peg(&client, network, wollet.tip().height()).unwrap();
    let pegin_address = wollet.pegin_address(Some(0), &fed_peg).unwrap();

    env.bitcoind_generate(101);
    let txid = env.bitcoind_sendtoaddress(pegin_address.address(), 100_000_000);
    // TODO: Fetch the Bitcoin transaction through a parent-chain Waterfalls client.
    let transaction = env.bitcoind_getrawtransaction(txid);

    env.bitcoind_generate(101);
    let proof_hex = env.bitcoind_gettxoutproof(txid);
    let proof = Vec::<u8>::from_hex(&proof_hex).unwrap();
    env.elementsd_generate(2);

    let funding = PeginFunding::from_raw(
        pegin_address,
        &bitcoin::consensus::serialize(&transaction),
        &proof,
    )
    .unwrap();
    let pegin_amount = funding.deposit().amount().to_sat();
    let destination = wollet.address(None).unwrap().address().clone();
    let mut pset = wollet
        .tx_builder()
        .add_pegin_input(PeginInput::from(funding))
        .unwrap()
        .drain_lbtc_to(destination)
        .finish()
        .unwrap();

    assert_eq!(signer.sign(&mut pset).unwrap(), 1);
    let transaction = wollet.finalize(&mut pset).unwrap();
    assert!(transaction.input[0].is_pegin());
    assert_eq!(transaction.input[0].witness.pegin_witness.len(), 6);
    let expected_balance = pegin_amount - transaction.fee_in(*network.policy_asset());
    let transaction_hex = elements::encode::serialize(&transaction).to_hex();
    let mempool_result =
        env.elementsd_call("testmempoolaccept", &[serde_json::json!([transaction_hex])]);
    assert!(
        mempool_result[0]["allowed"].as_bool().unwrap(),
        "{mempool_result}"
    );
    let liquid_txid = env.elementsd_sendrawtransaction(&transaction_hex);
    assert_eq!(liquid_txid, transaction.txid().to_string());
    env.elementsd_generate(1);

    let update = crate::wait_blockchain_tx_update(&mut client, &wollet);
    wollet.apply_update(update).unwrap();
    assert_eq!(
        *wollet
            .balance()
            .unwrap()
            .get(network.policy_asset())
            .unwrap(),
        expected_balance
    );
}

#[test]
fn test_fetch_full_header_regtest() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let client = test_client_electrum(&env.electrum_url());

    test_fetch_last_full_header(client, Network::default_regtest());
}

#[ignore = "require network calls"]
#[test]
fn test_fetch_full_header_mainnet() {
    let electrum_url = ElectrumUrl::new(LIQUID_SOCKET, true, true).unwrap();
    let electrum_client = ElectrumClient::new(&electrum_url).unwrap();
    test_fetch_last_full_header(electrum_client, Network::Liquid);
}

#[ignore = "require network calls"]
#[test]
fn test_fetch_full_header_testnet() {
    let electrum_url = ElectrumUrl::new(LIQUID_TESTNET_SOCKET, true, true).unwrap();
    let electrum_client = ElectrumClient::new(&electrum_url).unwrap();
    test_fetch_last_full_header(electrum_client, Network::TestnetLiquid);
}

fn test_fetch_last_full_header(mut client: ElectrumClient, network: Network) {
    let current_tip = client.tip().unwrap().height;
    let header = fetch_last_full_header(&client, network, current_tip).unwrap();

    let fed_peg_script = fed_peg_script(&header);
    assert!(fed_peg_script.is_some());
}
