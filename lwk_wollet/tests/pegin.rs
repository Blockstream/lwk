use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use elements::hex::ToHex;
use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};
use lwk_test_util::*;
use lwk_wollet::pegin::fetch_last_full_header;
use lwk_wollet::*;

#[test]
fn claim_pegin() {
    // TODO this test makes a pegin using the node as a reference implementation to implement the pegin
    // in the lwk wallet
    let env = TestEnvBuilder::from_env().with_bitcoind().build();

    env.bitcoind_generate(101);
    let (mainchain_address, claim_script) = env.elementsd_getpeginaddress();
    let txid = env.bitcoind_sendtoaddress(&mainchain_address, 100_000_000);
    let tx = env.bitcoind_getrawtransaction(txid);
    let tx_bytes = bitcoin::consensus::serialize(&tx);

    let pegin_vout = tx
        .output
        .iter()
        .position(|o| o.script_pubkey == mainchain_address.script_pubkey())
        .unwrap();

    env.bitcoind_generate(101);
    let proof = env.bitcoind_gettxoutproof(txid);

    env.elementsd_generate(2);

    let address_lbtc = env.elementsd_getnewaddress().to_string();

    let inputs = serde_json::json!([ {"txid":txid, "vout": pegin_vout,"pegin_bitcoin_tx": tx_bytes.to_hex(), "pegin_txout_proof": proof, "pegin_claim_script": claim_script } ]);
    let outputs = serde_json::json!([
        {address_lbtc: "0.9", "blinder_index": 0},
        {"fee": "0.1" }
    ]);

    let psbt = env.elementsd_raw_createpsbt(inputs, outputs);

    assert_eq!(env.elementsd_expected_next(&psbt), "updater");
    let psbt = env.elementsd_walletprocesspsbt(&psbt);
    assert_eq!(env.elementsd_expected_next(&psbt), "extractor");
    let tx_hex = env.elementsd_finalizepsbt(&psbt);
    let _txid = env.elementsd_sendrawtransaction(&tx_hex);
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
