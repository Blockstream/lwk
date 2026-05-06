use std::{collections::HashMap, str::FromStr};

use elements::{
    bitcoin::secp256k1::{Keypair, XOnlyPublicKey},
    hex::ToHex,
};
use elements_miniscript::{
    bitcoin::bip32::DerivationPath, ConfidentialDescriptor, DescriptorPublicKey,
};
use lwk_common::{singlesig_desc, Signer, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_jade::{
    register_multisig::{GetRegisteredMultisigParams, JadeDescriptor, RegisterMultisigParams},
    TestJadeEmulator,
};
use lwk_signer::{AnySigner, SignerError};
use lwk_simplicity::{
    scripts::{create_p2tr_address, load_program},
    signer::{finalize_transaction, get_sighash_all},
    simplicityhl::{
        num::U256, str::WitnessName, tracker::TrackerLogLevel, value::ValueConstructible,
        Arguments, Value, WitnessValues,
    },
};
use lwk_test_util::{init_logging, TestEnv, TestEnvBuilder, TEST_MNEMONIC};
use lwk_wollet::{blocking::BlockchainBackend, Network, WolletDescriptor, EC};

use crate::test_wollet::{generate_signer, multisig_desc, test_client_electrum, TestWollet};

pub fn jade_setup<'a>(docker: &'a Cli, mnemonic: &'a str) -> TestJadeEmulator<'a> {
    let mut test_jade_emul = TestJadeEmulator::new(docker);
    test_jade_emul.set_debug_mnemonic(mnemonic);
    test_jade_emul
}

fn roundtrip(
    env: &TestEnv,
    signers: &[&AnySigner],
    variant: Option<lwk_common::Singlesig>,
    threshold: Option<usize>,
) {
    let desc_str = match signers.len() {
        1 => singlesig_desc(
            signers[0],
            variant.unwrap(),
            lwk_common::DescriptorBlindingKey::Slip77,
        )
        .unwrap(),
        _ => {
            let desc = multisig_desc(signers, threshold.unwrap());
            register_multisig(signers, "custody", &desc);
            desc
        }
    };
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(env);

    let node_address = env.elementsd_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":2,\"ticker\":\"TEST\",\"version\":0}";
    let (asset, _token) = wallet.issueasset(signers, 10_000, 1, Some(contract), None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 10, &asset, None);
    let node_address = env.elementsd_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = env.elementsd_getnewaddress();
    let node_address2 = env.elementsd_getnewaddress();
    wallet.send_many(
        signers,
        &node_address1,
        &asset,
        &node_address2,
        &wallet.policy_asset(),
        None,
    );
}

fn emul_roundtrip_singlesig(variant: Singlesig) {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = &[&AnySigner::Jade(jade_init.jade, xpub_identifier)];
    roundtrip(&env, signers, Some(variant), None);
}

fn emul_roundtrip_multisig(threshold: usize) {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let sw_signer = generate_signer();
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = &[
        &AnySigner::Jade(jade_init.jade, xpub_identifier),
        &AnySigner::Software(sw_signer),
    ];
    roundtrip(&env, signers, None, Some(threshold));
}

fn sign_explicit_jade_input(env: &TestEnv, signer: &AnySigner) {
    let desc_str = singlesig_desc(
        signer,
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
    )
    .unwrap();
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_explicit(env, 100_000, None, None);
    let explicit_utxos = wallet.wollet.explicit_utxos().unwrap();
    assert_eq!(explicit_utxos.len(), 1);
    let external_utxo = explicit_utxos[0].clone();

    let node_address = env.elementsd_getnewaddress();
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 10_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    let sigs_added_or_overwritten = signer.sign(&mut pset).unwrap();
    assert!(sigs_added_or_overwritten > 0);

    let txid = wallet.send(&mut pset);
    assert!(wallet.wollet.transaction(&txid).unwrap().is_some());
}

fn sign_explicit_jade_output(env: &TestEnv, signer: &AnySigner) {
    let desc_str = singlesig_desc(
        signer,
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
    )
    .unwrap();
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(env);

    let mut explicit_address = env.elementsd_getnewaddress();
    explicit_address.blinding_pubkey = None;
    let mut pset = wallet
        .tx_builder()
        .add_explicit_recipient(&explicit_address, 10_000, wallet.policy_asset())
        .unwrap()
        .finish()
        .unwrap();

    let sigs_added_or_overwritten = signer.sign(&mut pset).unwrap();
    assert!(sigs_added_or_overwritten > 0);

    let txid = wallet.send(&mut pset);
    assert!(wallet.wollet.transaction(&txid).unwrap().is_some());
}

fn sign_mixed_jade_input(env: &TestEnv, signer: &AnySigner) {
    let jade_desc_str = singlesig_desc(
        signer,
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
    )
    .unwrap();
    let sw_signer = AnySigner::Software(generate_signer());
    let sw_desc_str = singlesig_desc(
        &sw_signer,
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
    )
    .unwrap();
    let client = test_client_electrum(&env.electrum_url());
    let mut jade_wallet = TestWollet::new(client, &jade_desc_str);
    let client = test_client_electrum(&env.electrum_url());
    let mut sw_wallet = TestWollet::new(client, &sw_desc_str);

    jade_wallet.fund_btc(env);
    sw_wallet.fund_btc(env);
    let utxo = sw_wallet.wollet.utxos().unwrap()[0].clone();
    let external_utxo = sw_wallet.make_external(&utxo);

    let node_address = env.elementsd_getnewaddress();
    let mut pset = jade_wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 110_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    sw_wallet.wollet.add_details(&mut pset).unwrap();

    let sw_fingerprint = sw_signer.fingerprint().unwrap();
    let jade_sigs = signer.sign(&mut pset).unwrap();
    assert!(jade_sigs > 0);
    assert!(pset.inputs().iter().all(|input| {
        !input
            .bip32_derivation
            .values()
            .any(|(fingerprint, _)| fingerprint == &sw_fingerprint)
            || input.partial_sigs.is_empty()
    }));
    let sw_sigs = sw_signer.sign(&mut pset).unwrap();
    assert!(sw_sigs > 0);

    let txid = jade_wallet.send(&mut pset);
    assert!(jade_wallet.wollet.transaction(&txid).unwrap().is_some());
}

fn sign_mixed_input_with_simplicity(env: &TestEnv, signer: &AnySigner) {
    let jade_desc_str = singlesig_desc(
        signer,
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
    )
    .unwrap();
    let client = test_client_electrum(&env.electrum_url());
    let mut jade_wallet = TestWollet::new(client, &jade_desc_str);
    jade_wallet.fund_explicit(env, 100_000, None, None);
    let jade_utxo = jade_wallet.wollet.explicit_utxos().unwrap()[0].clone();

    let simplicity_signer = generate_signer();
    let xprv = simplicity_signer
        .derive_xprv(&"m".parse().unwrap())
        .unwrap();
    let keypair = Keypair::from_secret_key(&EC, &xprv.private_key);
    let (xonly, _) = keypair.x_only_public_key();
    let mut args = HashMap::new();
    args.insert(
        WitnessName::from_str_unchecked("PUBLIC_KEY"),
        Value::u256(U256::from_byte_array(xonly.serialize())),
    );
    let program = load_program(
        include_str!("../../../lwk_simplicity/data/p2pk.simf"),
        Arguments::from(args),
    )
    .unwrap();
    let nums_internal_key = XOnlyPublicKey::from_str(
        "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
    )
    .unwrap();
    let simplicity_address = create_p2tr_address(
        program.commit().cmr(),
        &nums_internal_key,
        Network::default_regtest().address_params(),
    );
    let simplicity_script = simplicity_address.script_pubkey();
    let simplicity_desc = format!(":{}", simplicity_script.to_hex());
    let client = test_client_electrum(&env.electrum_url());
    let mut simplicity_wallet = TestWollet::new(client, &simplicity_desc);
    simplicity_wallet.fund_explicit(env, 100_000, None, None);
    let simplicity_utxo = simplicity_wallet.wollet.explicit_utxos().unwrap()[0].clone();

    let mut explicit_address = env.elementsd_getnewaddress();
    explicit_address.blinding_pubkey = None;
    let mut pset = jade_wallet
        .tx_builder()
        .add_explicit_recipient(&explicit_address, 10_000, jade_wallet.policy_asset())
        .unwrap()
        .add_external_utxos(vec![jade_utxo, simplicity_utxo])
        .unwrap()
        // Keep this PSET limited to the explicit external inputs above.
        .set_wallet_utxos(vec![])
        .finish()
        .unwrap();

    let simplicity_input_index = pset
        .inputs()
        .iter()
        .position(|input| {
            input
                .witness_utxo
                .as_ref()
                .is_some_and(|txout| txout.script_pubkey == simplicity_script)
        })
        .unwrap();
    let txouts = pset
        .inputs()
        .iter()
        .map(|input| input.witness_utxo.clone().unwrap())
        .collect::<Vec<_>>();

    let jade_sigs = signer.sign(&mut pset).unwrap();
    assert_eq!(jade_sigs, 1);

    let mut tx = jade_wallet.wollet.finalize(&mut pset).unwrap();
    let message = get_sighash_all(
        &tx,
        &program,
        &nums_internal_key,
        &txouts,
        simplicity_input_index,
        env.elementsd_network(),
    )
    .unwrap();
    let signature = EC.sign_schnorr(&message, &keypair);
    let mut witness_map = HashMap::new();
    witness_map.insert(
        WitnessName::from_str_unchecked("SIGNATURE"),
        Value::byte_array(signature.serialize()),
    );

    tx = finalize_transaction(
        tx,
        &program,
        &nums_internal_key,
        &txouts,
        simplicity_input_index,
        WitnessValues::from(witness_map),
        env.elementsd_network(),
        TrackerLogLevel::None,
    )
    .unwrap();

    let txid = jade_wallet.client.broadcast(&tx).unwrap();
    jade_wallet.wait_for_tx_outside_list(&txid);
    simplicity_wallet.wait_for_tx_outside_list(&txid);
}

fn reject_duplicate_jade_derivations(env: &TestEnv, signer: &AnySigner) {
    let fingerprint = signer.fingerprint().unwrap();
    let xpubs = ["48h/1h/0h/2h", "48h/1h/1h/2h"]
        .into_iter()
        .map(|path_str| {
            let path = DerivationPath::from_str(&format!("m/{path_str}")).unwrap();
            let xpub = signer.derive_xpub(&path).unwrap();
            (Some((fingerprint, path)), xpub)
        })
        .collect();

    let desc = lwk_common::multisig_desc(
        2,
        xpubs,
        lwk_common::Multisig::Wsh,
        lwk_common::DescriptorBlindingKey::Slip77Rand,
    )
    .unwrap();

    register_multisig(&[signer], "dupejade", &desc);
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);

    wallet.fund_btc(env);
    let node_address = env.elementsd_getnewaddress();
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 10_000)
        .unwrap()
        .finish()
        .unwrap();

    let err = signer.sign(&mut pset).unwrap_err();
    assert!(matches!(
        err,
        SignerError::JadeError(lwk_jade::Error::MultipleBip32DerivationsInput(0))
    ));
}

#[test]
fn emul_roundtrip_wpkh() {
    emul_roundtrip_singlesig(Singlesig::Wpkh);
}

#[test]
fn emul_roundtrip_shwpkh() {
    emul_roundtrip_singlesig(Singlesig::ShWpkh);
}

#[test]
fn emul_roundtrip_2of2() {
    emul_roundtrip_multisig(2);
}

#[test]
fn jade_slip77() {
    init_logging();
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);

    let script_variant = lwk_common::Singlesig::Wpkh;
    let blinding_variant = lwk_common::DescriptorBlindingKey::Slip77;
    let desc_str =
        lwk_common::singlesig_desc(&jade_init.jade, script_variant, blinding_variant).unwrap();
    assert!(desc_str.contains(lwk_test_util::TEST_MNEMONIC_SLIP77))
}

#[test]
fn emul_explicit() {
    init_logging();
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let jade = jade_setup(&docker, TEST_MNEMONIC);
    let id = jade.jade.identifier().unwrap();
    let jade_signer = AnySigner::Jade(jade.jade, id);

    sign_explicit_jade_input(&env, &jade_signer);
    sign_explicit_jade_output(&env, &jade_signer);
    sign_mixed_jade_input(&env, &jade_signer);
    sign_mixed_input_with_simplicity(&env, &jade_signer);
    reject_duplicate_jade_derivations(&env, &jade_signer);
}

fn multi_multisig(env: &TestEnv, jade_signer: &AnySigner) {
    // Signers: jade, sw1, sw2
    let sw_signer1 = AnySigner::Software(generate_signer());
    let sw_signer2 = AnySigner::Software(generate_signer());

    // Wallet multi1
    let signers_m1 = &[jade_signer, &sw_signer1];
    let desc = multisig_desc(signers_m1, 2);
    register_multisig(signers_m1, "multi1", &desc);
    let client = test_client_electrum(&env.electrum_url());
    let mut w1 = TestWollet::new(client, &desc);

    // Wallet multi2
    let signers_m2 = &[jade_signer, &sw_signer2];
    let desc = multisig_desc(signers_m2, 2);
    register_multisig(signers_m2, "multi2", &desc);
    let client = test_client_electrum(&env.electrum_url());
    let mut w2 = TestWollet::new(client, &desc);

    // Jade has now 2 registered multisigs

    // Fund multi1
    w1.fund_btc(env);

    // Spend from multi1 (with change)
    let node_address = env.elementsd_getnewaddress();
    w1.send_btc(signers_m1, None, Some((node_address, 10_000)));

    // Spend from multi1 to a change address of multi2 (with change)
    // (Jade shows both "change" outputs in this case)
    let w2_address = w2.wollet.change(None).unwrap().address().clone();

    let mut pset = w1
        .tx_builder()
        .add_lbtc_recipient(&w2_address, 10_000)
        .unwrap()
        .finish()
        .unwrap();

    w2.wollet.add_details(&mut pset).unwrap();
    for signer in signers_m1 {
        w1.sign(signer, &mut pset);
    }
    w1.send(&mut pset);
    w2.sync();
    assert!(w2.balance(&w2.policy_asset()) > 0);

    // Spend from multi2
    let node_address = env.elementsd_getnewaddress();
    w2.send_btc(signers_m2, None, Some((node_address, 1_000)));
}

#[test]
fn emul_multi_multisig() {
    init_logging();
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let jade = jade_setup(&docker, TEST_MNEMONIC);
    let id = jade.jade.identifier().unwrap();
    let jade_signer = AnySigner::Jade(jade.jade, id);
    multi_multisig(&env, &jade_signer);
}

#[cfg(feature = "serial")]
mod serial {
    use super::*;
    use lwk_jade::Jade;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_roundtrip() {
        let env = TestEnvBuilder::from_env().with_electrum().build();
        let network = lwk_common::Network::default_regtest();
        let ports = Jade::available_ports_with_jade();
        let port_name = &ports.first().unwrap().port_name;
        let jade = Jade::from_serial(network, port_name, None).unwrap();
        let id = jade.identifier().unwrap();
        let jade_signer = AnySigner::Jade(jade, id);
        let signers = &[&jade_signer];

        roundtrip(&env, signers, Some(Singlesig::Wpkh), None);
        roundtrip(&env, signers, Some(Singlesig::ShWpkh), None);
        // multisig
        let sw_signer = AnySigner::Software(generate_signer());
        let signers = &[&jade_signer, &sw_signer];
        roundtrip(&env, signers, None, Some(2));
    }

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_multi_multisig() {
        init_logging();
        let env = TestEnvBuilder::from_env().with_electrum().build();
        let network = lwk_common::Network::default_regtest();
        let ports = Jade::available_ports_with_jade();
        let port_name = &ports.first().unwrap().port_name;
        let jade = Jade::from_serial(network, port_name, None).unwrap();
        let id = jade.identifier().unwrap();
        let jade_signer = AnySigner::Jade(jade, id);
        multi_multisig(&env, &jade_signer);
    }

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial; confirm transaction on device screen"]
    fn jade_explicit() {
        init_logging();
        let env = TestEnvBuilder::from_env().with_electrum().build();
        let network = lwk_common::Network::default_regtest();
        let ports = Jade::available_ports_with_jade();
        let port_name = &ports.first().unwrap().port_name;
        let jade = Jade::from_serial(network, port_name, None).unwrap();
        let id = jade.identifier().unwrap();
        let jade_signer = AnySigner::Jade(jade, id);

        sign_explicit_jade_input(&env, &jade_signer);
        sign_explicit_jade_output(&env, &jade_signer);
        sign_mixed_jade_input(&env, &jade_signer);
        sign_mixed_input_with_simplicity(&env, &jade_signer);
    }
}

pub fn register_multisig(signers: &[&AnySigner], name: &str, desc: &str) {
    // Register a multisig descriptor on each *jade* signer
    let desc_orig: WolletDescriptor = desc.parse().unwrap();
    let desc: JadeDescriptor = desc_orig.ct_descriptor().unwrap().try_into().unwrap();
    let params = RegisterMultisigParams {
        network: lwk_common::Network::default_regtest(),
        multisig_name: name.into(),
        descriptor: desc,
    };

    let params_get = GetRegisteredMultisigParams {
        multisig_name: name.into(),
    };

    for signer in signers {
        if let AnySigner::Jade(s, _) = signer {
            s.register_multisig(params.clone()).unwrap();

            let r = s.get_registered_multisig(params_get.clone()).unwrap();
            let desc_elements =
                ConfidentialDescriptor::<DescriptorPublicKey>::try_from(&r.descriptor).unwrap();
            let desc_wollet = WolletDescriptor::try_from(desc_elements).unwrap();
            assert_eq!(desc_orig.to_string(), desc_wollet.to_string());
        }
    }
}
