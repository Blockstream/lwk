use std::str::FromStr;

use lwk_simplicity::lending::{LendingSession, OfferDetails};
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::elements::AssetId;
use lwk_wollet::*;

mod common;
use common::*;

pub fn fund_wollet<S: BlockchainBackend>(
    wollet: &mut Wollet,
    client: &mut S,
    env: &TestEnv,
    satoshi: u64,
) {
    let address = wollet.address(None).unwrap();
    let txid = env.elementsd_sendtoaddress(address.address(), satoshi, None);
    env.elementsd_generate(1);
    wait_for_tx(wollet, client, &txid);
}

#[test]
#[should_panic(expected = "not yet implemented")]
fn test_borrow_flow() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let mut client = electrum_client(&env);
    let network = env.elementsd_network();

    // Create borrower
    let borrower_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, borrower_signer.xpub());
    let borrower_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut borrower_wollet = WolletBuilder::new(network, borrower_wd.clone())
        .build()
        .unwrap();

    // Create lender
    let lender_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, lender_signer.xpub());
    let lender_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut _lender_wollet = WolletBuilder::new(network, lender_wd.clone())
        .build()
        .unwrap();

    // Fund the borrower wallet with L-BTC
    fund_wollet(&mut borrower_wollet, &mut client, &env, 500);
    // Create lending session
    let mut borrower_session = LendingSession::builder(network, borrower_wd)
        .set_indexer_url("https://127.0.0.1".to_string())
        .set_signer(Box::new(borrower_signer))
        .set_electrum_client(client)
        .build()
        .unwrap();

    // sync to fetch fee transaction
    borrower_session.sync().unwrap();

    // borrower_prepare, which selects fee UTXO, builds, signs, finalizes, broadcasts internally
    let prepared = borrower_session.borrower_prepare().unwrap();
    let client = electrum_client(&env);
    let transaction = client.get_transaction(prepared.txid).unwrap();

    assert_eq!(
        transaction.output[0].asset.to_string(),
        prepared.issued_asset_id.to_string()
    );

    assert_eq!(
        transaction.output[1].asset.to_string(),
        prepared.issued_asset_id.to_string()
    );

    // Create borrow details
    let borrow_details = OfferDetails {
        principal_asset_id: AssetId::from_byte_array([0; 32]),
        principal_amount: 10000,
        collateral_asset_id: *network.policy_asset(),
        collateral_amount: 200000,
        // 60 blocks after the current one
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        // 20 % interest rate
        principal_interest_rate: 2_000,
    };

    let _create = borrower_session
        .borrower_create_offer(borrow_details)
        .unwrap();
}
