pub mod change_detection;
pub mod init;

use bs_containers::testcontainers::clients::Cli;
use jade::get_receive_address::Variant;
use signer::Signer;

use crate::{
    test_jade::init::inner_jade_debug_initialization,
    test_session::{
        generate_signer, multisig_desc, setup, singlesig_desc, TestElectrumServer, TestWollet,
    },
    TEST_MNEMONIC,
};

fn roundtrip(
    server: &TestElectrumServer,
    signers: &[&Signer],
    variant: Option<Variant>,
    threshold: Option<usize>,
) {
    let desc_str = match signers.len() {
        1 => singlesig_desc(signers[0], variant.unwrap()),
        _ => multisig_desc(signers, threshold.unwrap()),
    };
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":2,\"ticker\":\"TEST\",\"version\":0}";
    let (asset, _token) = wallet.issueasset(signers, 1_000, 1, contract, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 10, &asset, None);
}

fn emul_roundtrip_singlesig(variant: Variant) {
    let server = setup();
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = &[&Signer::Jade(&jade_init.jade)];
    roundtrip(&server, signers, Some(variant), None);
}

fn emul_roundtrip_multisig(threshold: usize) {
    let server = setup();
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let sw_signer = generate_signer();
    let signers = &[&Signer::Jade(&jade_init.jade), &Signer::Software(sw_signer)];
    roundtrip(&server, signers, None, Some(threshold));
}

#[test]
fn emul_roundtrip_wpkh() {
    emul_roundtrip_singlesig(Variant::Wpkh);
}

#[test]
fn emul_roundtrip_shwpkh() {
    emul_roundtrip_singlesig(Variant::ShWpkh);
}

#[test]
fn emul_roundtrip_2of2() {
    emul_roundtrip_multisig(2);
}

#[cfg(feature = "serial")]
mod serial {
    use super::*;
    use crate::test_jade::init::serial;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_roundtrip() {
        let server = setup();
        let jade = serial::unlock();
        let signers = &[&Signer::Jade(&jade)];

        roundtrip(&server, signers, Some(Variant::Wpkh), None);
        roundtrip(&server, signers, Some(Variant::ShWpkh), None);
        // multisig
        // let sw_signer = generate_signer();
        // let signers = &[&Signer::Jade(&jade), &Signer::Software(sw_signer)];
        // FIXME: register multisig
        // roundtrip(&server, signers, None, Some(2));
    }
}
