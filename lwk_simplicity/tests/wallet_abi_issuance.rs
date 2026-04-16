use lwk_simplicity::wallet_abi::schema::{
    AssetVariant, BlinderVariant, InputIssuance, InputIssuanceKind, InputSchema, LockVariant,
    OutputSchema, PreviewOutputKind, RuntimeParams, TxCreateRequest, TxEvaluateRequest,
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
