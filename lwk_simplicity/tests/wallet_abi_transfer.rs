use lwk_simplicity::wallet_abi::schema::{
    AssetVariant, BlinderVariant, LockVariant, OutputSchema, RuntimeParams, TxCreateRequest,
    TxEvaluateRequest,
};

#[path = "common/wallet_abi.rs"]
mod wallet_abi_common;

use wallet_abi_common::WalletAbiLiveHarness;

#[test]
fn wallet_abi_transfer_lbtc() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);

    let recipient = harness.env.elementsd_getnewaddress();
    let policy_asset = harness.network.policy_asset();

    let params = RuntimeParams {
        inputs: vec![],
        outputs: vec![OutputSchema {
            id: "external".to_string(),
            amount_sat: 25_000,
            lock: LockVariant::Script {
                script: recipient.script_pubkey(),
            },
            asset: AssetVariant::AssetId {
                asset_id: policy_asset,
            },
            blinder: BlinderVariant::Explicit,
        }],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let request_id = uuid::Uuid::new_v4().to_string();
    let evaluate_request =
        TxEvaluateRequest::from_parts(&request_id, harness.network, params.clone())
            .expect("evaluate request");
    let evaluate_response = harness
        .evaluate_request(evaluate_request)
        .expect("evaluate request");
    let preview = evaluate_response.preview.expect("preview");
    let preview_delta = preview
        .asset_deltas
        .iter()
        .find(|delta| delta.asset_id == policy_asset)
        .expect("policy asset preview delta")
        .wallet_delta_sat;

    let process_request = TxCreateRequest::from_parts(&request_id, harness.network, params, true)
        .expect("process request");
    let process_response = harness
        .process_request(process_request)
        .expect("process request");
    let process_preview = process_response
        .preview()
        .expect("process preview accessor")
        .expect("process preview");
    assert_eq!(process_preview, preview);

    let txid = process_response.transaction.expect("transaction info").txid;
    harness.mine_and_sync_sender(1);

    let wallet_tx = harness
        .sender_transaction(&txid)
        .expect("sender wallet tx after broadcast");
    assert_eq!(
        *wallet_tx
            .balance
            .get(&policy_asset)
            .expect("policy asset balance"),
        preview_delta,
    );
}
