use indexer::*;
use lwk_common::Signer;
use std::str::FromStr;
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
    let mut b_wollet = WolletBuilder::new(network, borrower_wd.clone())
        .build()
        .unwrap();

    // Create lender
    let lender_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, lender_signer.xpub());
    let lender_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut l_wollet = WolletBuilder::new(network, lender_wd.clone())
        .build()
        .unwrap();

    // Issue assets
    let collateral = env.elementsd_issueasset(1_000_000);
    let principal = env.elementsd_issueasset(1_000_000);
    // this is separate NFT for protocol fee keeper (service/indexer maintainer)
    let protocol_fee_keeper_asset_id = PROTOCOL_FEE_KEEPER_ASSET_ID;

    // Fund borrower with L-BTC and collateral asset
    fund_wollet(&mut b_wollet, &mut client, &env, 500_000, Some(collateral));
    fund_wollet(&mut b_wollet, &mut client, &env, 500_000, None);

    // Fund lender with L-BTC and principal asset
    fund_wollet(&mut l_wollet, &mut client, &env, 500_000, None);
    fund_wollet(&mut l_wollet, &mut client, &env, 10_000, Some(principal));

    // Create lending session for borrower
    let mut borrower_session = LendingSession::builder(network, borrower_wd.clone())
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(client)
        .build()
        .unwrap();

    let mut client = electrum_client(&env);

    borrower_session.sync().unwrap();

    // Prepare borrower for offer creation
    // This would create a 'Factory' simplicity covenant as a standalone asset,
    // which would ensure that offers would be created properly, eliminating the risk of locking coins.
    let prepared = borrower_session
        .borrower_prepare(BorrowerAccountParams {})
        .unwrap();
    let mut pset = prepared.inner().clone();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let txid = client.broadcast(&tx).unwrap();
    let transaction = client.get_transaction(txid).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    // Check if indexer is showing our factory by script_pubkey
    let spk = transaction.output[0].script_pubkey.to_hex();
    let factory = wait_factory(spk, &indexer_client).await;

    // Create borrow details
    let borrow_details = OfferDetails {
        principal_asset_id: principal,
        principal_amount: 10_000,
        collateral_asset_id: collateral,
        collateral_amount: 200_000,
        // 60 blocks after the current one
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        // 20 % interest rate
        principal_interest_rate: 2_000,
        // Asset ID of the indexer maintainer, which serves as authentication.
        // Some percentage of the repaid principal (right now it is 10%) would be locked
        // in the simplicity covenant, which can be unlocked only with this asset ID.
        // Without that, the indexer wouldn't see an offer.
        protocol_fee_keeper_asset_id,
    };

    // Create an offer with given details. This would create a borrower NFT, which would be sent
    // to the borrower's address, and a lender NFT, which would be locked with a simplicity covenant
    // until the offer is accepted. Locked collateral from the borrower locked to the Lending
    // simplicity covenant.
    let create = borrower_session
        .borrower_create_offer(borrow_details, factory)
        .unwrap();

    let mut pset = create.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let creation_txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    // Check if indexer is showing our offer.
    let offer = wait_offer(OfferStatus::Pending, None, &indexer_client).await;
    let item = offer;

    let lender_client = electrum_client(&env);

    // Create LendingSession for lender
    let mut lender_session = LendingSession::builder(network, lender_wd)
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(lender_client)
        .build()
        .unwrap();

    lender_session.sync().unwrap();

    // Accept pending offer by its TXID.
    // This would lock the principal with a simplicity covenant, which could be unlocked with the borrower
    // NFT, send a lender NFT to the lender's address and change the offer status of the Lending
    // simplicity covenant to Active.
    // Note that we don't need a borrower to accept.
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
    borrower_session.sync().unwrap();

    // Verify the offer status changed to Active in the indexer
    wait_offer(OfferStatus::Active, Some(item.id.clone()), &indexer_client).await;

    // Claim principal as the borrower which would send principal to the borrower's address.
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

    // Check if principal present in the wallet
    let balance = borrower_session.wollet().balance().unwrap();
    let principal_balance = balance.get(&principal).copied().unwrap_or(0);
    assert!(
        principal_balance >= 10000,
        "borrower should have received the principal: got {principal_balance}, expected at least 10000"
    );

    // Fund borrower with principal asset for repayment
    fund_wollet(&mut b_wollet, &mut client, &env, 100_000, Some(principal));
    borrower_session.sync().unwrap();

    // Fully repay the loan as a borrower.
    // This would send principal to the lender vault, protocol fee keeper vault and return
    // locked collateral to the borrower.
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
    let repayment_txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);

    // Verify the offer status changed to Repaid in the indexer
    wait_offer(OfferStatus::Repaid, Some(item.id.clone()), &indexer_client).await;

    // Lender claims the principal from the finalized vault after full repayment
    lender_session.sync().unwrap();

    let claim_repayment = lender_session
        .lender_final_claim(ClaimRepaymentDetails {
            lending_creation_txid: creation_txid,
            repayment_txid,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();

    let mut pset = claim_repayment.into_inner();
    lender_signer.sign(&mut pset).unwrap();
    let tx = lender_session.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    lender_session.sync().unwrap();

    let balance = lender_session.wollet().balance().unwrap();
    let principal_balance = balance.get(&principal).copied().unwrap_or(0);
    assert_eq!(
        principal_balance,
        // starting principal amount + 20% interest rate - 10% from interest for protocol keeper
        11800,
        "lender should have received principal from the finalized vault"
    );
}

#[tokio::test]
async fn test_cancel_offer() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let mut client = electrum_client(&env);
    let cli = Cli::default();
    let (indexer_client, _indexer_ctx) = launch_indexer(&env, &cli).await;
    let network = env.elementsd_network();

    let borrower_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, borrower_signer.xpub());
    let borrower_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut b_wollet = WolletBuilder::new(network, borrower_wd.clone())
        .build()
        .unwrap();

    let collateral = env.elementsd_issueasset(1_000_000);
    let principal = env.elementsd_issueasset(1_000_000);
    let protocol_fee_keeper_asset_id = PROTOCOL_FEE_KEEPER_ASSET_ID;

    fund_wollet(&mut b_wollet, &mut client, &env, 200_000, Some(collateral));
    fund_wollet(&mut b_wollet, &mut client, &env, 500_000, None);

    let mut borrower_session = LendingSession::builder(network, borrower_wd.clone())
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(client)
        .build()
        .unwrap();

    let client = electrum_client(&env);

    borrower_session.sync().unwrap();

    let prepared = borrower_session
        .borrower_prepare(BorrowerAccountParams {})
        .unwrap();
    let mut pset = prepared.inner().clone();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let txid = client.broadcast(&tx).unwrap();
    let transaction = client.get_transaction(txid).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    let spk = transaction.output[0].script_pubkey.to_hex();
    let factory = wait_factory(spk, &indexer_client).await;

    let borrow_details = OfferDetails {
        principal_asset_id: principal,
        principal_amount: 10_000,
        collateral_asset_id: collateral,
        collateral_amount: 200_000,
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        principal_interest_rate: 2_000,
        protocol_fee_keeper_asset_id,
    };

    let create = borrower_session
        .borrower_create_offer(borrow_details, factory)
        .unwrap();
    let mut pset = create.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let creation_txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    let item = wait_offer(OfferStatus::Pending, None, &indexer_client).await;

    // Cancel the pending offer
    let cancel = borrower_session
        .cancel_offer(CancelOfferDetails {
            creation_txid,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();
    let mut pset = cancel.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    // Verify collateral is back
    let balance = borrower_session.wollet().balance().unwrap();
    let collateral_balance = balance.get(&collateral).copied().unwrap_or(0);
    assert!(
        collateral_balance >= 200_000,
        "borrower should have received collateral back after cancellation"
    );

    // Verify the offer status changed to Cancelled
    wait_offer(
        OfferStatus::Cancelled,
        Some(item.id.clone()),
        &indexer_client,
    )
    .await;
}

#[tokio::test]
async fn test_liquidate_offer() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let mut client = electrum_client(&env);
    let cli = Cli::default();
    let (indexer_client, _indexer_ctx) = launch_indexer(&env, &cli).await;
    let network = env.elementsd_network();

    let borrower_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, borrower_signer.xpub());
    let borrower_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut b_wollet = WolletBuilder::new(network, borrower_wd.clone())
        .build()
        .unwrap();

    let lender_signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, lender_signer.xpub());
    let lender_wd = WolletDescriptor::from_str(&desc).unwrap();
    let mut l_wollet = WolletBuilder::new(network, lender_wd.clone())
        .build()
        .unwrap();

    let collateral = env.elementsd_issueasset(1_000_000);
    let principal = env.elementsd_issueasset(1_000_000);
    let protocol_fee_keeper_asset_id = PROTOCOL_FEE_KEEPER_ASSET_ID;

    fund_wollet(&mut b_wollet, &mut client, &env, 200_000, Some(collateral));
    fund_wollet(&mut b_wollet, &mut client, &env, 500_000, None);

    fund_wollet(&mut l_wollet, &mut client, &env, 500_000, None);
    fund_wollet(&mut l_wollet, &mut client, &env, 100_000, Some(principal));

    let mut borrower_session = LendingSession::builder(network, borrower_wd.clone())
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(client)
        .build()
        .unwrap();

    let mut client = electrum_client(&env);

    borrower_session.sync().unwrap();

    let prepared = borrower_session
        .borrower_prepare(BorrowerAccountParams {})
        .unwrap();
    let mut pset = prepared.inner().clone();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let txid = client.broadcast(&tx).unwrap();
    let transaction = client.get_transaction(txid).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    let spk = transaction.output[0].script_pubkey.to_hex();
    let factory = wait_factory(spk, &indexer_client).await;

    let borrow_details = OfferDetails {
        principal_asset_id: principal,
        principal_amount: 10_000,
        collateral_asset_id: collateral,
        collateral_amount: 200_000,
        loan_expiration_time: env.elementsd_height() as u32 + 10,
        principal_interest_rate: 2_000,
        protocol_fee_keeper_asset_id,
    };

    let create = borrower_session
        .borrower_create_offer(borrow_details, factory)
        .unwrap();
    let mut pset = create.into_inner();
    borrower_signer.sign(&mut pset).unwrap();
    let tx = borrower_session.finalize(&mut pset).unwrap();
    let creation_txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    borrower_session.sync().unwrap();

    let item = wait_offer(OfferStatus::Pending, None, &indexer_client).await;

    let lender_client = electrum_client(&env);

    let mut lender_session = LendingSession::builder(network, lender_wd)
        .set_indexer_url(indexer_client.base_url().into())
        .set_electrum_client(lender_client)
        .build()
        .unwrap();

    lender_session.sync().unwrap();

    let accept = lender_session
        .accept_offer(AcceptOfferDetails {
            pending_offer_creation_txid: creation_txid,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();
    let mut pset = accept.into_inner();
    lender_signer.sign(&mut pset).unwrap();
    let tx = lender_session.finalize(&mut pset).unwrap();
    let acceptance_txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    lender_session.sync().unwrap();

    wait_offer(OfferStatus::Active, Some(item.id.clone()), &indexer_client).await;

    let covenant_outpoint = lwk_wollet::elements::OutPoint {
        txid: acceptance_txid,
        vout: 0,
    };

    // liquidate before expiration should fail to broadcast
    lender_session
        .liquidate_offer(LiquidateOfferDetails {
            active_covenant_outpoint: covenant_outpoint,
            protocol_fee_keeper_asset_id,
        })
        .err()
        .expect("Liquidation should fail before expiration time");

    // Mine blocks past the expiration height
    env.elementsd_generate(15);
    lender_session.sync().unwrap();

    // Wait for the client to fetch new mined blocks, because we are relying on this info in
    // liquidation method.
    let current_height = env.elementsd_height() as u32;
    for _ in 0..20 {
        let tip = client.tip().unwrap();
        if tip.height == current_height {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let liquidate = lender_session
        .liquidate_offer(LiquidateOfferDetails {
            active_covenant_outpoint: covenant_outpoint,
            protocol_fee_keeper_asset_id,
        })
        .unwrap();
    let mut pset = liquidate.into_inner();
    lender_signer.sign(&mut pset).unwrap();
    let tx = lender_session.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    lender_session.sync().unwrap();

    // Verify lender received the collateral
    let balance = lender_session.wollet().balance().unwrap();
    let collateral_balance = balance.get(&collateral).copied().unwrap_or(0);
    assert!(
        collateral_balance >= 200_000,
        "lender should have received collateral from liquidation"
    );

    wait_offer(OfferStatus::Liquidated, Some(item.id), &indexer_client).await;
}
