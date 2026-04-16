use std::collections::HashMap;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

use lwk_simplicity::scripts::{create_p2tr_address, load_program};
use lwk_simplicity::simplicityhl::{
    num::U256, str::WitnessName, value::ValueConstructible, Arguments, Value, WitnessValues,
};
use lwk_simplicity::wallet_abi::schema::{
    serialize_arguments, serialize_witness, AssetVariant, BlinderVariant, FinalizerSpec,
    InputSchema, InputUnblinding, InternalKeySource, LockVariant, OutputSchema, PreviewOutputKind,
    RuntimeParams, RuntimeSimfWitness, SimfArguments, SimfWitness, TxCreateRequest,
    TxEvaluateRequest, UTXOSource,
};
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::hex::{FromHex, ToHex};
use lwk_wollet::elements::Transaction;
use lwk_wollet::{Chain, ElectrumClient, ElectrumUrl, WolletBuilder, WolletDescriptor};
#[path = "common/wallet_abi.rs"]
mod wallet_abi_common;

use wallet_abi_common::WalletAbiLiveHarness;

#[test]
fn wallet_abi_spends_p2pk_simf_input() {
    let mut harness = WalletAbiLiveHarness::new();
    harness.fund_sender_lbtc(100_000);
    let source = include_str!("../data/p2pk.simf");
    let policy_asset = harness.network.policy_asset();
    let internal_key = InternalKeySource::Bip0341;

    let mut resolved_arguments = HashMap::new();
    resolved_arguments.insert(
        WitnessName::from_str_unchecked("PUBLIC_KEY"),
        Value::u256(U256::from_byte_array(
            harness.sender_xonly_public_key.serialize(),
        )),
    );
    let program =
        load_program(source, Arguments::from(resolved_arguments.clone())).expect("program");
    let p2pk_address = create_p2tr_address(
        program.commit().cmr(),
        &internal_key.get_x_only_pubkey(),
        harness.network.address_params(),
    );

    let descriptor =
        WolletDescriptor::from_str(&format!(":{}", p2pk_address.script_pubkey().to_hex()))
            .expect("p2pk descriptor");
    let mut p2pk_wallet = WolletBuilder::new(harness.network, descriptor)
        .build()
        .expect("p2pk wallet");
    let electrum_url = ElectrumUrl::from_str(&harness.env.electrum_url()).expect("electrum url");
    let mut client = ElectrumClient::new(&electrum_url).expect("electrum client");

    let funding_txid = harness
        .env
        .elementsd_sendtoaddress(&p2pk_address, 100_000, None);
    harness.env.elementsd_generate(1);
    for _ in 0..120 {
        if let Some(update) = client.full_scan(&p2pk_wallet).expect("p2pk scan") {
            p2pk_wallet.apply_update(update).expect("p2pk update");
        }
        if p2pk_wallet
            .transactions()
            .expect("p2pk transactions")
            .iter()
            .any(|tx| tx.txid == funding_txid)
        {
            break;
        }
        sleep(Duration::from_millis(500));
    }
    assert!(p2pk_wallet
        .transactions()
        .expect("p2pk transactions")
        .iter()
        .any(|tx| tx.txid == funding_txid));

    let provided_input = p2pk_wallet
        .explicit_utxos()
        .expect("explicit utxos")
        .into_iter()
        .next()
        .expect("p2pk input");

    let params = RuntimeParams {
        inputs: vec![InputSchema {
            id: "p2pk".into(),
            utxo_source: UTXOSource::Provided {
                outpoint: provided_input.outpoint,
            },
            unblinding: InputUnblinding::Explicit,
            finalizer: FinalizerSpec::Simf {
                source_simf: source.to_owned(),
                internal_key: internal_key.clone(),
                arguments: serialize_arguments(&SimfArguments::new(Arguments::from(
                    resolved_arguments,
                )))
                .expect("arguments"),
                witness: serialize_witness(&SimfWitness {
                    resolved: WitnessValues::from(HashMap::<WitnessName, Value>::new()),
                    runtime_arguments: vec![RuntimeSimfWitness::SigHashAll {
                        name: "SIGNATURE".into(),
                        public_key: harness.sender_xonly_public_key,
                    }],
                })
                .expect("witness"),
            },
            ..InputSchema::default()
        }],
        outputs: vec![OutputSchema {
            id: "wallet_receive".into(),
            amount_sat: 50_000,
            lock: LockVariant::Wallet,
            asset: AssetVariant::AssetId {
                asset_id: policy_asset,
            },
            blinder: BlinderVariant::Wallet,
        }],
        fee_rate_sat_kvb: None,
        lock_time: None,
    };

    let request_id = "6eb5292b-595d-4998-9470-2dff682d1bcb";
    let evaluate_response = harness
        .evaluate_request(
            TxEvaluateRequest::from_parts(request_id, harness.network, params.clone())
                .expect("evaluate request"),
        )
        .expect("evaluate request");
    let preview = evaluate_response.preview.expect("preview");
    assert!(preview.outputs.iter().any(|output| {
        output.kind == PreviewOutputKind::Receive
            && output.asset_id == policy_asset
            && output.amount_sat == 50_000
    }));

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
    assert!(!tx.input[0].witness.script_witness.is_empty());
    assert_eq!(tx.output.iter().filter(|output| output.is_fee()).count(), 1);
    assert!(tx
        .output
        .iter()
        .filter(|output| !output.is_fee())
        .all(|output| output.value.explicit().is_none()));

    harness.mine_and_sync_sender(1);

    let sender_tx = harness
        .sender_transaction(&transaction.txid)
        .expect("sender transaction");
    assert!(sender_tx.outputs.iter().flatten().any(|output| {
        output.ext_int == Chain::External
            && output.unblinded.asset == policy_asset
            && output.unblinded.value == 50_000
    }));
}
