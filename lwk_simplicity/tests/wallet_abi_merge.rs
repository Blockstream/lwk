use lwk_simplicity::wallet_abi::schema::{
    AssetVariant, BlinderVariant, LockVariant, OutputSchema, RuntimeParams,
};
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::hex::FromHex;
use lwk_wollet::elements::{Transaction, TxOut};

#[path = "common/wallet_abi.rs"]
mod wallet_abi_common;

use wallet_abi_common::WalletAbiLiveHarness;

#[test]
fn wallet_abi_merge_lbtc_inputs() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(30_000);
    harness.fund_sender_lbtc(35_000);

    let recipient = harness.env.elementsd_getnewaddress();
    let policy_asset = harness.network.policy_asset();
    let params = RuntimeParams {
        inputs: vec![],
        outputs: vec![OutputSchema {
            id: "merged".to_string(),
            amount_sat: 50_000,
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
    let (_, response) = harness
        .evaluate_then_process(&request_id, params)
        .expect("request roundtrip");
    let transaction = response.transaction.expect("transaction");
    let tx_bytes = Vec::<u8>::from_hex(&transaction.tx_hex).expect("transaction hex");
    let tx: Transaction = deserialize(&tx_bytes).expect("transaction decode");

    assert_eq!(tx.input.len(), 2);

    let requested_outputs: Vec<&TxOut> = tx
        .output
        .iter()
        .filter(|output| !output.is_fee() && output.value.explicit().is_some())
        .collect();
    assert_eq!(requested_outputs.len(), 1);
    assert_eq!(
        requested_outputs[0].script_pubkey,
        recipient.script_pubkey()
    );
    assert_eq!(requested_outputs[0].value.explicit(), Some(50_000));
    assert_eq!(tx.output.iter().filter(|output| output.is_fee()).count(), 1);
    assert_eq!(tx.output.len(), 3);

    harness.mine_and_sync_sender(1);
    assert!(harness.sender_transaction(&transaction.txid).is_some());
}
