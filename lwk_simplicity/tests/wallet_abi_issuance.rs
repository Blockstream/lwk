use lwk_simplicity::wallet_abi::schema::{
    AssetFilter, AssetVariant, BlinderVariant, InputIssuance, InputIssuanceKind, InputSchema,
    LockVariant, OutputSchema, PreviewOutputKind, RuntimeParams, TxCreateRequest,
    TxEvaluateRequest, UTXOSource, WalletSourceFilter,
};

#[path = "common/wallet_abi.rs"]
mod wallet_abi_common;

use wallet_abi_common::WalletAbiLiveHarness;

#[test]
fn wallet_abi_new_issuance() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);

    let params = RuntimeParams {
        inputs: vec![InputSchema::new("issuance").with_issuance(InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 5,
            token_amount_sat: 1,
            entropy: [7; 32],
        })],
        outputs: vec![
            OutputSchema {
                id: "issued_asset".into(),
                amount_sat: 5,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            },
            OutputSchema {
                id: "reissuance_token".into(),
                amount_sat: 1,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceToken { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            },
        ],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let request_id = "78e8809f-c3de-4f46-a270-ae4ed8dd51c5";
    let evaluate_response = harness
        .evaluate_request(
            TxEvaluateRequest::from_parts(request_id, harness.network, params.clone())
                .expect("evaluate request"),
        )
        .expect("evaluate request");
    let preview = evaluate_response.preview.expect("preview");

    let issued_asset = preview
        .outputs
        .iter()
        .find(|output| output.kind == PreviewOutputKind::Receive && output.amount_sat == 5)
        .expect("issued asset preview")
        .asset_id;
    let reissuance_token = preview
        .outputs
        .iter()
        .find(|output| output.kind == PreviewOutputKind::Receive && output.amount_sat == 1)
        .expect("reissuance token preview")
        .asset_id;

    assert_ne!(issued_asset, reissuance_token);
    assert_eq!(
        preview
            .asset_deltas
            .iter()
            .find(|delta| delta.asset_id == issued_asset)
            .expect("issued asset delta")
            .wallet_delta_sat,
        5
    );
    assert_eq!(
        preview
            .asset_deltas
            .iter()
            .find(|delta| delta.asset_id == reissuance_token)
            .expect("reissuance token delta")
            .wallet_delta_sat,
        1
    );

    let response = harness
        .process_request(
            TxCreateRequest::from_parts(request_id, harness.network, params, true)
                .expect("process request"),
        )
        .expect("process request");

    assert_eq!(
        response
            .preview()
            .expect("process preview accessor")
            .expect("process preview"),
        preview
    );

    let transaction = response.transaction.expect("transaction");
    harness.mine_and_sync_sender(1);

    let sender_tx = harness
        .sender_transaction(&transaction.txid)
        .expect("sender transaction");
    assert_eq!(sender_tx.type_, "issuance");
    let sender_balance = harness.sender_wallet.balance().expect("sender balance");
    assert_eq!(sender_balance.get(&issued_asset), Some(&5));
    assert_eq!(sender_balance.get(&reissuance_token), Some(&1));

    let issuance = harness
        .sender_wallet
        .issuance(&issued_asset)
        .expect("wallet issuance");
    assert_eq!(issuance.asset, issued_asset);
    assert_eq!(issuance.token, reissuance_token);
    assert_eq!(issuance.asset_amount, Some(5));
    assert_eq!(issuance.token_amount, Some(1));
    assert!(!issuance.is_reissuance);
}

#[test]
fn wallet_abi_reissuance() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);

    let initial_params = RuntimeParams {
        inputs: vec![InputSchema::new("issuance").with_issuance(InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 5,
            token_amount_sat: 1,
            entropy: [7; 32],
        })],
        outputs: vec![
            OutputSchema {
                id: "issued_asset".into(),
                amount_sat: 5,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            },
            OutputSchema {
                id: "reissuance_token".into(),
                amount_sat: 1,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceToken { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            },
        ],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let issuance_response = harness
        .process_request(
            TxCreateRequest::from_parts(
                "406d1c2c-6927-4f78-a9ce-f54b5ec9bb9e",
                harness.network,
                initial_params,
                true,
            )
            .expect("issuance request"),
        )
        .expect("issuance request");
    let issuance_txid = issuance_response.transaction.expect("issuance tx").txid;
    harness.mine_and_sync_sender(1);

    let issuance = harness
        .sender_wallet
        .issuances()
        .expect("sender issuances")
        .into_iter()
        .find(|details| details.txid == issuance_txid)
        .expect("issuance details");

    let params = RuntimeParams {
        inputs: vec![InputSchema {
            id: "reissuance".into(),
            utxo_source: UTXOSource::Wallet {
                filter: WalletSourceFilter {
                    asset: AssetFilter::Exact {
                        asset_id: issuance.token,
                    },
                    ..WalletSourceFilter::default()
                },
            },
            issuance: Some(InputIssuance {
                kind: InputIssuanceKind::Reissue,
                asset_amount_sat: 7,
                token_amount_sat: 0,
                entropy: issuance.entropy,
            }),
            ..InputSchema::default()
        }],
        outputs: vec![OutputSchema {
            id: "reissued_asset".into(),
            amount_sat: 7,
            lock: LockVariant::Wallet,
            asset: AssetVariant::ReIssuanceAsset { input_index: 0 },
            blinder: BlinderVariant::Wallet,
        }],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let request_id = "9de5cd8d-c344-4e95-8798-7be733fb1fd1";
    let evaluate_response = harness
        .evaluate_request(
            TxEvaluateRequest::from_parts(request_id, harness.network, params.clone())
                .expect("evaluate request"),
        )
        .expect("evaluate request");
    let preview = evaluate_response.preview.expect("preview");

    assert_eq!(
        preview
            .asset_deltas
            .iter()
            .find(|delta| delta.asset_id == issuance.asset)
            .expect("reissued asset delta")
            .wallet_delta_sat,
        7
    );
    assert!(preview
        .asset_deltas
        .iter()
        .all(|delta| delta.asset_id != issuance.token));

    let response = harness
        .process_request(
            TxCreateRequest::from_parts(request_id, harness.network, params, true)
                .expect("process request"),
        )
        .expect("process request");

    assert_eq!(
        response
            .preview()
            .expect("process preview accessor")
            .expect("process preview"),
        preview
    );

    let transaction = response.transaction.expect("transaction");
    harness.mine_and_sync_sender(1);

    let sender_tx = harness
        .sender_transaction(&transaction.txid)
        .expect("sender transaction");
    assert_eq!(sender_tx.type_, "reissuance");

    let sender_balance = harness.sender_wallet.balance().expect("sender balance");
    assert_eq!(sender_balance.get(&issuance.asset), Some(&12));
    assert_eq!(sender_balance.get(&issuance.token), Some(&1));

    let reissuance = harness
        .sender_wallet
        .issuances()
        .expect("sender issuances")
        .into_iter()
        .find(|details| details.txid == transaction.txid)
        .expect("reissuance details");
    assert!(reissuance.is_reissuance);
    assert_eq!(reissuance.asset, issuance.asset);
    assert_eq!(reissuance.token, issuance.token);
    assert_eq!(reissuance.asset_amount, Some(7));
    assert_eq!(reissuance.token_amount, None);
}
