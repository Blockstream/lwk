use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use elements::hex::{FromHex, ToHex};
use elements::OutPoint;
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
    let signer = SwSigner::new_with_network(TEST_MNEMONIC, network).unwrap();
    let descriptor = format!(
        "ct(slip77({TEST_MNEMONIC_SLIP77}),elwpkh({}/*))",
        signer.xpub()
    );
    let client = clients::blocking::WaterfallsClient::new(&env.waterfalls_url(), network).unwrap();
    let mut wallet = TestWollet::with_opt(
        client,
        &descriptor,
        &TestWolletOpt {
            network: Some(network),
            ..Default::default()
        },
    );

    let fed_peg = fetch_fed_peg(&wallet.client, network, wallet.tip().height()).unwrap();
    let pegin_address = wallet.wollet.pegin_address(Some(0), &fed_peg).unwrap();

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
    let destination = wallet.address();
    let pegin_input = PeginInput::from(funding);

    assert!(matches!(
        TxBuilder::new(Network::Liquid).add_pegin_input(pegin_input.clone()),
        Err(Error::PeginNetworkMismatch { .. })
    ));

    let builder = wallet
        .tx_builder()
        .add_pegin_input(pegin_input.clone())
        .unwrap();
    assert!(matches!(
        builder.add_pegin_input(pegin_input.clone()),
        Err(Error::DuplicatedOutpoint(_, context)) if context == "pegin inputs"
    ));

    assert!(matches!(
        wallet
            .tx_builder()
            .add_pegin_input(pegin_input.clone())
            .unwrap()
            .set_wallet_utxos(vec![])
            .set_inputs_order(vec![])
            .finish(),
        Err(Error::PeginUnsupportedBuilderMode("manual inputs order"))
    ));

    assert!(matches!(
        wallet
            .tx_builder()
            .add_pegin_input(pegin_input.clone())
            .unwrap()
            .liquidex_make(OutPoint::null(), &destination, 1, *network.policy_asset(),)
            .unwrap()
            .finish(),
        Err(Error::PeginUnsupportedBuilderMode("LiquiDEX"))
    ));

    let mut pset = wallet
        .tx_builder()
        .add_pegin_input(pegin_input)
        .unwrap()
        .drain_lbtc_to(&destination)
        .unwrap()
        .finish()
        .unwrap();

    let pegin_utxo = pset.inputs()[0].witness_utxo.as_ref().unwrap();
    assert!(matches!(
        pegin_utxo.asset,
        elements::confidential::Asset::Explicit(_)
    ));
    assert!(matches!(
        pegin_utxo.value,
        elements::confidential::Value::Explicit(_)
    ));

    assert_eq!(signer.sign(&mut pset).unwrap(), 1);
    let transaction = wallet.wollet.finalize(&mut pset).unwrap();
    assert!(transaction.input[0].is_pegin());
    assert_eq!(transaction.input[0].witness.pegin_witness.len(), 6);
    let destination_output = transaction
        .output
        .iter()
        .find(|output| !output.script_pubkey.is_empty())
        .unwrap();
    assert!(matches!(
        destination_output.asset,
        elements::confidential::Asset::Confidential(_)
    ));
    assert!(matches!(
        destination_output.value,
        elements::confidential::Value::Confidential(_)
    ));
    let fee_output = transaction
        .output
        .iter()
        .find(|output| output.script_pubkey.is_empty())
        .unwrap();
    assert!(matches!(
        fee_output.asset,
        elements::confidential::Asset::Explicit(_)
    ));
    assert!(matches!(
        fee_output.value,
        elements::confidential::Value::Explicit(_)
    ));
    let expected_balance = pegin_amount - transaction.fee_in(*network.policy_asset());
    let transaction_hex = elements::encode::serialize(&transaction).to_hex();
    assert!(env.elementsd_testmempoolaccept(&transaction_hex));
    let liquid_txid = env.elementsd_sendrawtransaction(&transaction_hex);
    assert_eq!(liquid_txid, transaction.txid().to_string());
    env.elementsd_generate(1);

    let update = crate::wait_blockchain_tx_update(&mut wallet.client, &wallet.wollet);
    wallet.wollet.apply_update(update).unwrap();
    assert_eq!(wallet.balance_btc(), expected_balance);
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
