use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use lwk_common::{singlesig_desc, Signer, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_jade::{
    register_multisig::{GetRegisteredMultisigParams, JadeDescriptor, RegisterMultisigParams},
    TestJadeEmulator,
};
use lwk_signer::AnySigner;
use lwk_test_util::{init_logging, TEST_MNEMONIC, TestEnv, TestEnvBuilder};
use lwk_wollet::WolletDescriptor;

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
        let network = lwk_common::Network::LocaltestLiquid;
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
        let network = lwk_common::Network::LocaltestLiquid;
        let ports = Jade::available_ports_with_jade();
        let port_name = &ports.first().unwrap().port_name;
        let jade = Jade::from_serial(network, port_name, None).unwrap();
        let id = jade.identifier().unwrap();
        let jade_signer = AnySigner::Jade(jade, id);
        multi_multisig(&env, &jade_signer);
    }
}

pub fn register_multisig(signers: &[&AnySigner], name: &str, desc: &str) {
    // Register a multisig descriptor on each *jade* signer
    let desc_orig: WolletDescriptor = desc.parse().unwrap();
    let desc: JadeDescriptor = desc_orig.as_ref().try_into().unwrap();
    let params = RegisterMultisigParams {
        network: lwk_common::Network::LocaltestLiquid,
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
