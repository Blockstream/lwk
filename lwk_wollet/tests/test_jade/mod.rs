use lwk_common::{singlesig_desc, Signer, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_signer::AnySigner;
use lwk_test_util::{
    generate_signer, init_logging,
    jade::{TestJadeEmulator, TestMutexJadeEmulator},
    multisig_desc, register_multisig, setup, TestElectrumServer, TestWollet, TEST_MNEMONIC,
};

pub fn jade_setup<'a>(docker: &'a Cli, mnemonic: &'a str) -> TestMutexJadeEmulator<'a> {
    let mut test_jade_emul = TestJadeEmulator::new(docker);
    test_jade_emul.set_debug_mnemonic(mnemonic);
    TestMutexJadeEmulator::new(test_jade_emul)
}

fn roundtrip(
    server: &TestElectrumServer,
    signers: &[&AnySigner],
    variant: Option<lwk_common::Singlesig>,
    threshold: Option<usize>,
) {
    let desc_str = match signers.len() {
        1 => singlesig_desc(
            signers[0],
            variant.unwrap(),
            lwk_common::DescriptorBlindingKey::Slip77,
            false,
        )
        .unwrap(),
        _ => {
            let desc = multisig_desc(signers, threshold.unwrap());
            register_multisig(signers, "custody", &desc);
            desc
        }
    };
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":2,\"ticker\":\"TEST\",\"version\":0}";
    let (asset, _token) = wallet.issueasset(signers, 10_000, 1, contract, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 10, &asset, None);
    let node_address = server.node_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = server.node_getnewaddress();
    let node_address2 = server.node_getnewaddress();
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
    let server = setup(false);
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = &[&AnySigner::Jade(jade_init.jade, xpub_identifier)];
    roundtrip(&server, signers, Some(variant), None);
}

fn emul_roundtrip_multisig(threshold: usize) {
    let server = setup(false);
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let sw_signer = generate_signer();
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = &[
        &AnySigner::Jade(jade_init.jade, xpub_identifier),
        &AnySigner::Software(sw_signer),
    ];
    roundtrip(&server, signers, None, Some(threshold));
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
        lwk_common::singlesig_desc(&jade_init.jade, script_variant, blinding_variant, false)
            .unwrap();
    assert!(desc_str.contains(lwk_test_util::TEST_MNEMONIC_SLIP77))
}

#[cfg(feature = "serial")]
mod serial {
    use super::*;
    use lwk_jade::{mutex_jade::MutexJade, Network};

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_roundtrip() {
        let server = setup(false);
        let network = Network::LocaltestLiquid;
        let ports = MutexJade::available_ports_with_jade();
        let port_name = &ports.first().unwrap().port_name;
        let jade = MutexJade::from_serial(network, port_name, None).unwrap();
        let id = jade.identifier().unwrap();
        let jade_signer = AnySigner::Jade(jade, id);
        let signers = &[&jade_signer];

        roundtrip(&server, signers, Some(Singlesig::Wpkh), None);
        roundtrip(&server, signers, Some(Singlesig::ShWpkh), None);
        // multisig
        let sw_signer = AnySigner::Software(generate_signer());
        let signers = &[&jade_signer, &sw_signer];
        roundtrip(&server, signers, None, Some(2));
    }
}
