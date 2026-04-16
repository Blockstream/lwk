use std::str::FromStr;

use lwk_common::Signer as _;
use lwk_simplicity::wallet_abi::schema::{
    AssetVariant, BlinderVariant, LockVariant, OutputSchema, RuntimeParams, TxCreateRequest,
    TxEvaluateRequest,
};
use lwk_test_util::generate_slip77;
use lwk_wollet::bitcoin::bip32::DerivationPath;
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::hex::FromHex;
use lwk_wollet::elements::{Transaction, TxOut};
use lwk_wollet::{Chain, WolletBuilder, WolletDescriptor};

#[path = "common/mod.rs"]
mod common;
#[path = "common/wallet_abi.rs"]
mod wallet_abi_common;

use common::{electrum_client, generate_signer, wait_for_tx};
use wallet_abi_common::WalletAbiLiveHarness;

const WALLET_ACCOUNT_PATH: &str = "m/84h/1h/0h";

#[test]
fn wallet_abi_blinds_wallet_and_script_outputs() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);

    let recipient_signer = generate_signer();
    let account_path = DerivationPath::from_str(WALLET_ACCOUNT_PATH).expect("account path");
    let account_xpub = recipient_signer
        .derive_xpub(&account_path)
        .expect("account xpub");
    let recipient_descriptor = format!(
        "ct(slip77({}),elwpkh([{}/84h/1h/0h]{}/<0;1>/*))",
        generate_slip77(),
        recipient_signer.fingerprint(),
        account_xpub
    );
    let recipient_descriptor =
        WolletDescriptor::from_str(&recipient_descriptor).expect("recipient descriptor");
    let mut recipient_wallet = WolletBuilder::new(harness.network, recipient_descriptor)
        .build()
        .expect("recipient wallet");
    let mut recipient_client = electrum_client(&harness.env);
    let recipient_address = recipient_wallet
        .address(None)
        .expect("recipient address")
        .address()
        .clone();

    let policy_asset = harness.network.policy_asset();
    let params = RuntimeParams {
        inputs: vec![],
        outputs: vec![
            OutputSchema {
                id: "wallet_receive".into(),
                amount_sat: 10_000,
                lock: LockVariant::Wallet,
                asset: AssetVariant::AssetId {
                    asset_id: policy_asset,
                },
                blinder: BlinderVariant::Wallet,
            },
            OutputSchema {
                id: "script_blinded".into(),
                amount_sat: 12_000,
                lock: LockVariant::Script {
                    script: recipient_address.script_pubkey(),
                },
                asset: AssetVariant::AssetId {
                    asset_id: policy_asset,
                },
                blinder: BlinderVariant::Provided {
                    pubkey: recipient_address
                        .blinding_pubkey
                        .expect("recipient blinding pubkey"),
                },
            },
        ],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let request_id = "f4505c4b-d19d-4472-8d1c-118902926698";
    let evaluate_response = harness
        .evaluate_request(
            TxEvaluateRequest::from_parts(request_id, harness.network, params.clone())
                .expect("evaluate request"),
        )
        .expect("evaluate request");
    let preview = evaluate_response.preview.expect("preview");

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
    let tx_bytes = Vec::<u8>::from_hex(&transaction.tx_hex).expect("transaction hex");
    let tx: Transaction = deserialize(&tx_bytes).expect("transaction decode");
    assert_eq!(tx.input.len(), 1);
    assert_eq!(tx.output.iter().filter(|output| output.is_fee()).count(), 1);

    let non_fee_outputs: Vec<&TxOut> = tx.output.iter().filter(|output| !output.is_fee()).collect();
    assert!(non_fee_outputs.len() >= 3);
    assert!(non_fee_outputs
        .iter()
        .all(|output| output.value.explicit().is_none()));
    assert!(non_fee_outputs
        .iter()
        .any(|output| { output.script_pubkey == recipient_address.script_pubkey() }));

    harness.mine_and_sync_sender(1);
    wait_for_tx(
        &mut recipient_wallet,
        &mut recipient_client,
        &transaction.txid,
    );

    let sender_tx = harness
        .sender_transaction(&transaction.txid)
        .expect("sender transaction");
    assert!(sender_tx.outputs.iter().flatten().any(|output| {
        output.ext_int == Chain::External
            && output.unblinded.asset == policy_asset
            && output.unblinded.value == 10_000
    }));

    let recipient_balance = recipient_wallet.balance().expect("recipient balance");
    assert_eq!(recipient_balance.get(&policy_asset), Some(&12_000));
}
