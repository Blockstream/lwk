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
fn wallet_abi_split_lbtc() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);

    let first_recipient = harness.env.elementsd_getnewaddress();
    let second_recipient = harness.env.elementsd_getnewaddress();
    let policy_asset = harness.network.policy_asset();
    let params = RuntimeParams {
        inputs: vec![],
        outputs: vec![
            OutputSchema {
                id: "first".to_string(),
                amount_sat: 15_000,
                lock: LockVariant::Script {
                    script: first_recipient.script_pubkey(),
                },
                asset: AssetVariant::AssetId {
                    asset_id: policy_asset,
                },
                blinder: BlinderVariant::Explicit,
            },
            OutputSchema {
                id: "second".to_string(),
                amount_sat: 20_000,
                lock: LockVariant::Script {
                    script: second_recipient.script_pubkey(),
                },
                asset: AssetVariant::AssetId {
                    asset_id: policy_asset,
                },
                blinder: BlinderVariant::Explicit,
            },
        ],
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

    assert_eq!(tx.input.len(), 1);

    let requested_outputs: Vec<&TxOut> = tx
        .output
        .iter()
        .filter(|output| !output.is_fee() && output.value.explicit().is_some())
        .collect();
    assert_eq!(requested_outputs.len(), 2);
    assert!(requested_outputs.iter().any(|output| {
        output.script_pubkey == first_recipient.script_pubkey()
            && output.value.explicit() == Some(15_000)
    }));
    assert!(requested_outputs.iter().any(|output| {
        output.script_pubkey == second_recipient.script_pubkey()
            && output.value.explicit() == Some(20_000)
    }));
    assert_eq!(tx.output.iter().filter(|output| output.is_fee()).count(), 1);
    assert_eq!(tx.output.len(), 4);

    harness.mine_and_sync_sender(1);
    assert!(harness.sender_transaction(&transaction.txid).is_some());
}
