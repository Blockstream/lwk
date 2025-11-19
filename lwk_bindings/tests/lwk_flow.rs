use lwk::{Address, ElectrumClient, Mnemonic, Network, Signer, Txid, Wollet};
use lwk_wollet::ElementsNetwork;
use std::str::FromStr;

// TODO move in e2e tests
#[test]
fn test_lwk_flow() {
    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    let network: Network = ElementsNetwork::default_regtest().into();
    let signer = Signer::new(&Mnemonic::new(mnemonic).unwrap(), &network).unwrap();

    let env = lwk_test_util::TestEnvBuilder::from_env()
        .with_electrum()
        .build();

    let singlesig_desc = signer.wpkh_slip77_descriptor().unwrap();

    let electrum_client = ElectrumClient::from_url(&env.electrum_url()).unwrap();

    let wollet = Wollet::new(&network, &singlesig_desc, None).unwrap();
    let _latest_address = wollet.address(None); // lastUnused
    let address_0 = wollet.address(Some(0)).unwrap();
    let expected_address_0 = "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq";
    assert_eq!(expected_address_0, address_0.address().to_string());

    let txid = env.elementsd_sendtoaddress(
        &elements::Address::from_str(expected_address_0).unwrap(),
        100000000,
        None,
    );
    let txid = Txid::from_str(&txid.to_string()).unwrap();
    let _tx = wollet.wait_for_tx(&txid, &electrum_client).unwrap();

    let address_1 = wollet.address(Some(1)).unwrap();
    let expected_address_1 = "el1qqv8pmjjq942l6cjq69ygtt6gvmdmhesqmzazmwfsq7zwvan4kewdqmaqzegq50r2wdltkfsw9hw20zafydz4sqljz0eqe0vhc";
    assert_eq!(expected_address_1, address_1.address().to_string());

    let balance = wollet.balance();
    println!("{:?}", balance);
    let txs = wollet.transactions().unwrap();
    for tx in txs {
        for output in tx.outputs() {
            let script_pubkey = match output.as_ref() {
                Some(out) => out.script_pubkey().to_string(),
                None => "Not a spendable scriptpubkey".to_string(),
            };
            let value = match output.as_ref() {
                Some(out) => out.unblinded().value(),
                None => 0,
            };
            println!("script_pubkey: {:?}, value: {}", script_pubkey, value)
        }
    }

    let out_address = Address::new(expected_address_1).unwrap();
    let satoshis = 900;
    let fee_rate = 280_f32; // this seems like absolute fees

    let builder = network.tx_builder();
    builder.add_lbtc_recipient(&out_address, satoshis).unwrap();
    builder.fee_rate(Some(fee_rate)).unwrap();
    let pset = builder.finish(&wollet).unwrap();

    let signed_pset = signer.sign(&pset).unwrap();
    let finalized_pset = wollet.finalize(&signed_pset).unwrap();
    let txid = electrum_client
        .broadcast(&finalized_pset.extract_tx().unwrap())
        .unwrap();
    println!("BROADCASTED TX!\nTXID: {:?}", txid);

    let asset = env.elementsd_issueasset(10000000);
    let txid =
        env.elementsd_sendtoaddress(&expected_address_1.parse().unwrap(), 100000, Some(asset));
    let txid = Txid::from_str(&txid.to_string()).unwrap();
    let _tx = wollet.wait_for_tx(&txid, &electrum_client).unwrap();

    let builder = network.tx_builder();
    builder
        .add_recipient(&out_address, 100, &asset.into())
        .unwrap();
    builder.fee_rate(Some(fee_rate)).unwrap();
    let pset = builder.finish(&wollet).unwrap();

    let signed_pset = signer.sign(&pset).unwrap();
    let finalized_pset = wollet.finalize(&signed_pset).unwrap();
    let txid = electrum_client
        .broadcast(&finalized_pset.extract_tx().unwrap())
        .unwrap();
    println!("BROADCASTED TX!\nTXID: {:?}", txid);
}
