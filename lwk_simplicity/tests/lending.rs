use indexer::*;
use lwk_common::Signer;
use std::str::FromStr;
use std::time::Duration;
use testcontainers::clients::Cli;

use elements::hex::ToHex;
use lwk_simplicity::lending::*;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::*;

mod common;
mod indexer;
use common::*;

#[tokio::test]
async fn test_borrow_flow() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let mut client = electrum_client(&env);
    let cli = Cli::default();
    let (indexer_client, _indexer_ctx) = launch_indexer(&env, &cli).await;
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
    let mut lender_wollet = WolletBuilder::new(network, lender_wd.clone())
        .build()
        .unwrap();

    // Fund the borrower wallet with L-BTC
    fund_wollet(&mut borrower_wollet, &mut client, &env, 500_000, None);

    // Issue assets
    let collateral_asset_id = env.elementsd_issueasset(1_000_000);
    let principal_asset_id = env.elementsd_issueasset(1_000_000);
    // this is separate NFT for protocol fee keeper (service/indexer maintainer)
    let protocol_fee_keeper_asset_id = PROTOCOL_FEE_KEEPER_ASSET_ID;

    // Fund borrower with collateral asset
    fund_wollet(
        &mut borrower_wollet,
        &mut client,
        &env,
        500_000,
        Some(collateral_asset_id),
    );

    // Fund lender with L-BTC and principal asset
    fund_wollet(&mut lender_wollet, &mut client, &env, 500_000, None);
    fund_wollet(
        &mut lender_wollet,
        &mut client,
        &env,
        100_000,
        Some(principal_asset_id),
    );

    // Create lending session for borrower
    let mut borrower_session = LendingSession::builder(network, borrower_wd.clone())
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(client)
        .build()
        .unwrap();

    let mut client = electrum_client(&env);

    // sync to fetch fee transaction
    borrower_session.sync().unwrap();

    // borrower_prepare, which selects fee UTXO and builds transaction
    let prepared = borrower_session
        .borrower_prepare(BorrowerAccountParams {})
        .unwrap();
    let mut pset = prepared.inner().clone();

    // sign
    borrower_signer.sign(&mut pset).unwrap();

    // finalize
    let tx = borrower_session.finalize(&mut pset).unwrap();

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
        principal_asset_id,
        principal_amount: 10000,
        collateral_asset_id,
        collateral_amount: 200000,
        // 60 blocks after the current one
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        // 20 % interest rate
        principal_interest_rate: 2_000,
        protocol_fee_keeper_asset_id,
    };

    // sync to fetch fee transaction
    borrower_session.sync().unwrap();

    let create = borrower_session
        .borrower_create_offer(borrow_details, factory.clone().try_into().unwrap())
        .unwrap();

    let mut pset = create.into_inner();

    // sign
    borrower_signer.sign(&mut pset).unwrap();

    // finalize
    let tx = borrower_session.finalize(&mut pset).unwrap();

    // broadcast
    let creation_txid = client.broadcast(&tx).unwrap();

    env.elementsd_generate(1);

    // Sync to pick up L-BTC and collateral change from creation tx
    borrower_session.sync().unwrap();

    // Check if indexer is showing our factory by script_pubkey
    let offer = wait_offer(OfferStatus::Pending, None, &indexer_client).await;
    let item = offer;

    assert_eq!(item.issuance_factory_id, factory.id);
    assert_eq!(item.created_at_txid, creation_txid);

    let lender_client = electrum_client(&env);

    // create LendingSession for lender
    let mut lender_session = LendingSession::builder(network, lender_wd)
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(lender_client)
        .build()
        .unwrap();

    lender_session.sync().unwrap();

    let accept = lender_session
        .accept_offer(AcceptOfferDetails {
            pending_offer_creation_txid: creation_txid,
            protocol_fee_keeper_asset_id: PROTOCOL_FEE_KEEPER_ASSET_ID,
        })
        .unwrap();

    let mut pset = accept.into_inner();
    lender_signer.sign(&mut pset).unwrap();
    let tx = lender_session.finalize(&mut pset).unwrap();
    let acceptance_txid = client.broadcast(&tx).unwrap();

    env.elementsd_generate(1);

    // Verify the offer status changed to Active in the indexer
    wait_offer(OfferStatus::Active, Some(item.id), &indexer_client).await;

    // Claim principal as borrower
    borrower_session.sync().unwrap();

    let claim = borrower_session
        .claim_principal(ClaimPrincipalDetails {
            acceptance_txid,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();

    let mut pset = claim.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();

    env.elementsd_generate(1);

    borrower_session.sync().unwrap();

    let balance = borrower_session.wollet().balance().unwrap();
    let principal_balance = balance.get(&principal_asset_id).copied().unwrap_or(0);
    assert!(
        principal_balance >= 10000,
        "borrower should have received the principal: got {principal_balance}, expected at least 10000"
    );

    // Repay the loan
    // Fund borrower with principal asset for repayment
    fund_wollet(
        &mut borrower_wollet,
        &mut client,
        &env,
        100_000,
        Some(principal_asset_id),
    );

    borrower_session.sync().unwrap();

    let covenant_outpoint = lwk_wollet::elements::OutPoint {
        txid: acceptance_txid,
        vout: 0,
    };

    let repay = borrower_session
        .fully_repay_loan(RepaymentDetails {
            active_covenant_outpoint: covenant_outpoint,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();

    let mut pset = repay.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();

    env.elementsd_generate(1);
    wait_offer(OfferStatus::Repaid, Some(item.id), &indexer_client).await;
}
