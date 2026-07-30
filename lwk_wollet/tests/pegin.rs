use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use elements::hex::{FromHex, ToHex};
use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};
use lwk_common::Signer;
use lwk_signer::SwSigner;
use lwk_test_util::*;
use lwk_wollet::pegin::fetch_last_full_header;
use lwk_wollet::*;

#[test]
fn claim_pegin() {
    let env = TestEnvBuilder::from_env()
        .with_bitcoind()
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
    let wollet = WolletBuilder::new(network, descriptor).build().unwrap();

    let tip = env.elementsd_height() as u32;
    let epoch_length = network.dynamic_epoch_length();
    let full_header_height = (tip / epoch_length) * epoch_length;
    let block_hash = env.elementsd_call("getblockhash", &[full_header_height.into()]);
    let header_hex = env.elementsd_call("getblockheader", &[block_hash, false.into()]);
    let header_bytes = Vec::<u8>::from_hex(header_hex.as_str().unwrap()).unwrap();
    let header: elements::BlockHeader = elements::encode::deserialize(&header_bytes).unwrap();
    let fed_peg = FedPeg::from_block_header(network, &header).unwrap();
    let pegin_address = wollet.pegin_address(Some(0), &fed_peg).unwrap();

    env.bitcoind_generate(101);
    let txid = env.bitcoind_sendtoaddress(pegin_address.address(), 100_000_000);
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
    let destination = env.elementsd_getnewaddress();
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
    let transaction_hex = elements::encode::serialize(&transaction).to_hex();
    let mempool_result =
        env.elementsd_call("testmempoolaccept", &[serde_json::json!([transaction_hex])]);
    assert!(
        mempool_result[0]["allowed"].as_bool().unwrap(),
        "{mempool_result}"
    );
    assert_eq!(
        env.elementsd_sendrawtransaction(&transaction_hex),
        transaction.txid().to_string()
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
