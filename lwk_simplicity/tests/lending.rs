use lwk_common::Signer;
use std::str::FromStr;
use std::time::Duration;

use elements::hex::ToHex;
use lwk_simplicity::lending::*;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::elements::AssetId;
use lwk_wollet::*;

mod common;
mod indexer;
use common::*;

use testcontainers::clients::Cli;

pub fn fund_wollet<S: BlockchainBackend>(
    wollet: &mut Wollet,
    client: &mut S,
    env: &TestEnv,
    satoshi: u64,
    asset_id: Option<AssetId>,
) {
    let address = wollet.address(None).unwrap();
    let txid = env.elementsd_sendtoaddress(address.address(), satoshi, asset_id);
    env.elementsd_generate(1);
    wait_for_tx(wollet, client, &txid);
}

#[tokio::test]
async fn test_borrow_flow() {
    let binary = std::fs::canonicalize(
        std::env::var("LENDING_INDEXER_EXEC").expect("LENDING_INDEXER_EXEC must be set"),
    )
    .expect("LENDING_INDEXER_EXEC path does not exist");

    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let mut client = electrum_client(&env);

    // Start postgres, run migrations, launch indexer
    let cli = Cli::default();
    let indexer = indexer::start_indexer(&env, &cli, &binary, 8081).await;
    let indexer_url = indexer.api_url().to_string();
    let indexer_client = IndexerClient::builder(indexer_url.clone()).build().unwrap();

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
    fund_wollet(&mut borrower_wollet, &mut client, &env, 500_000, None);

    // create a collateral_asset and send to the borrower
    let collateral_asset_id = env.elementsd_issueasset(1_000_000);
    fund_wollet(
        &mut borrower_wollet,
        &mut client,
        &env,
        500_000,
        Some(collateral_asset_id),
    );

    // Create lending session
    let mut borrower_session = LendingSession::builder(network, borrower_wd)
        .set_indexer_url(indexer_url)
        .set_electrum_client(client)
        .build()
        .unwrap();

    let client = electrum_client(&env);

    // sync to fetch fee transaction
    borrower_session.sync().unwrap();

    // borrower_prepare, which selects fee UTXO and builds transaction
    let mut prepared = borrower_session
        .borrower_prepare(BorrowerAccountParams {})
        .unwrap();

    // sign
    borrower_signer.sign(&mut prepared.pset).unwrap();

    // finalize
    let tx = borrower_session.finalize(&mut prepared.pset).unwrap();

    // broadcast
    let txid = client.broadcast(&tx).unwrap();
    let transaction = client.get_transaction(txid).unwrap();

    env.elementsd_generate(1);

    assert_eq!(
        transaction.output[0].asset.to_string(),
        prepared.issued_asset_id.to_string()
    );

    assert_eq!(
        transaction.output[1].asset.to_string(),
        prepared.issued_asset_id.to_string()
    );

    // Check if indexer is showing our factory by script_pubkey
    let spk = transaction.output[0].script_pubkey.to_hex();
    let mut found_factory = None;

    for _ in 0..20 {
        let factories = indexer_client.get_factories_by_script(&spk).await.unwrap();

        // If we get an element, store it and break out of the loop
        if let Some(factory) = factories.into_iter().next() {
            found_factory = Some(factory);
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let factory = found_factory.expect("Factory was not found within the timeout");

    // Create borrow details
    let borrow_details = OfferDetails {
        principal_asset_id: AssetId::from_byte_array([0; 32]),
        principal_amount: 10000,
        collateral_asset_id,
        collateral_amount: 200000,
        // 60 blocks after the current one
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        // 20 % interest rate
        principal_interest_rate: 2_000,
        protocol_fee_keeper_asset_id: *network.policy_asset(),
    };

    // sync to fetch fee transaction
    borrower_session.sync().unwrap();

    let mut create = borrower_session
        .borrower_create_offer(borrow_details, factory.clone())
        .unwrap();

    // sign
    borrower_signer.sign(&mut create.pset).unwrap();

    // finalize
    let tx = borrower_session.finalize(&mut create.pset).unwrap();

    // broadcast
    let txid = client.broadcast(&tx).unwrap();

    env.elementsd_generate(1);

    // Check if indexer is showing our factory by script_pubkey
    let mut found_offers = None;

    for _ in 0..20 {
        let offers = indexer_client
            .list_offers(&OfferFiltersRequest::default())
            .await
            .unwrap();

        if !offers.items.is_empty() {
            found_offers = Some(offers);
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let items = found_offers.expect("offer not found").items;

    let item = items.first().expect("items for list_offers is empty");

    assert_eq!(item.issuance_factory_id, factory.id);
    assert_eq!(item.created_at_txid, txid.to_string());
}
