use std::collections::HashMap;
use std::str::FromStr;

use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::*;

use lwk_simplicity::simplicityhl::{
    num::U256, str::WitnessName, tracker::TrackerLogLevel, value::ValueConstructible, Arguments,
    Value, WitnessValues,
};
use lwk_simplicity::{
    scripts::{create_p2tr_address, load_program},
    signer::{finalize_transaction, get_sighash_all},
};

use elements::bitcoin::secp256k1::Keypair;
use elements::hex::ToHex;

mod common;
use common::*;

#[test]
fn test_simplicity_p2pk() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let network = ElementsNetwork::default_regtest();
    let params = network.address_params();
    let signer = generate_signer();
    let mut client = electrum_client(&env);
    let genesis_block_hash = env.elementsd_genesis_block_hash();

    // Read p2pk.simf
    let source = include_str!("../data/p2pk.simf");

    // Derive x-only key from signer
    let xprv = signer.derive_xprv(&"m".parse().unwrap()).unwrap();
    let keypair = Keypair::from_secret_key(&EC, &xprv.private_key);
    let (xonly, _) = keypair.x_only_public_key();

    // Compile program with PUBLIC_KEY argument
    let mut args = HashMap::new();
    args.insert(
        WitnessName::from_str_unchecked("PUBLIC_KEY"),
        Value::u256(U256::from_byte_array(xonly.serialize())),
    );
    let arguments = Arguments::from(args);
    let program = load_program(source, arguments).unwrap();

    // Create p2tr address
    let cmr = program.commit().cmr();
    let address = create_p2tr_address(cmr, &xonly, params);
    let spk = address.script_pubkey();

    // Create Wollet with the spk (no private blinding key)
    let desc_str = format!(":{}", spk.to_hex());
    let wd = WolletDescriptor::from_str(&desc_str).unwrap();
    let mut wollet = WolletBuilder::new(network, wd).build().unwrap();
    assert_eq!(wollet.address(Some(0)).unwrap().address(), &address);

    // Fund the p2tr address
    let sats_fund = 100_000;
    let txid = env.elementsd_sendtoaddress(&address, sats_fund, None);
    env.elementsd_generate(1);
    wait_for_tx(&mut wollet, &mut client, &txid);

    // Check that the Wollet has an explicit_utxo
    let explicit_utxos = wollet.explicit_utxos().unwrap();
    assert_eq!(explicit_utxos.len(), 1);
    let utxo = &explicit_utxos[0];
    assert_eq!(utxo.unblinded.value, sats_fund);
    let txouts = vec![utxo.txout.clone()];

    // Construct a PSET that spends such UTXO
    let node_address = env.elementsd_getnewaddress();
    let sats_send = 50_000;

    let pset = wollet
        .tx_builder()
        .add_external_utxos(explicit_utxos)
        .unwrap()
        .add_lbtc_recipient(&node_address, sats_send)
        .unwrap()
        .finish()
        .unwrap();
    let tx = pset.extract_tx().unwrap();
    let fee = tx.output.last().unwrap().value.explicit().unwrap();

    // Compute message and sign
    let input_index = 0;
    let message = get_sighash_all(
        &tx,
        &program,
        &xonly,
        &txouts,
        input_index,
        params,
        genesis_block_hash,
    )
    .unwrap();

    let signature = EC.sign_schnorr(&message, &keypair);

    // Add signature to the transaction
    let mut witness_map = HashMap::new();
    witness_map.insert(
        WitnessName::from_str_unchecked("SIGNATURE"),
        Value::byte_array(signature.serialize()),
    );
    let witness_values = WitnessValues::from(witness_map);

    let log_level = TrackerLogLevel::None;
    let tx = finalize_transaction(
        tx,
        &program,
        &xonly,
        &txouts,
        input_index,
        witness_values,
        params,
        genesis_block_hash,
        log_level,
    )
    .unwrap();

    // Broadcast the transaction
    let txid = client.broadcast(&tx).unwrap();
    env.elementsd_generate(1);
    wait_for_tx(&mut wollet, &mut client, &txid);

    let explicit_utxos = wollet.explicit_utxos().unwrap();
    let balance: u64 = explicit_utxos.iter().map(|u| u.unblinded.value).sum();
    assert_eq!(sats_fund - sats_send - fee, balance);
}
