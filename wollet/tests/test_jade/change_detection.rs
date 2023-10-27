use bs_containers::testcontainers::clients::Cli;
use elements::bitcoin::bip32::DerivationPath;
use jade::{
    get_receive_address::Variant,
    mutex_jade::MutexJade,
    register_multisig::{JadeDescriptor, RegisterMultisigParams},
};
use signer::Signer;
use std::{convert::TryInto, str::FromStr};
use wollet::WolletDescriptor;

use crate::{
    test_jade::init::inner_jade_debug_initialization,
    test_session::{generate_signer, generate_slip77, setup, TestWollet},
    TEST_MNEMONIC,
};

#[cfg(feature = "serial")]
mod serial {
    use jade::get_receive_address::Variant;
    use signer::Signer;

    use super::{send_lbtc, send_lbtc_multisig};
    use crate::test_jade::init::serial;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_send_lbtc_singlesig() {
        let jade = serial::unlock();

        let signers = [&Signer::Jade(&jade)];

        send_lbtc(&signers, Variant::Wpkh);
        send_lbtc(&signers, Variant::ShWpkh);
    }

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_send_lbtc_multisig() {
        let jade = serial::unlock();

        send_lbtc_multisig(jade);
    }
}

#[test]
fn emul_send_lbtc_singlesig() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = [&Signer::Jade(&jade_init.jade)];

    send_lbtc(&signers, Variant::Wpkh);
    send_lbtc(&signers, Variant::ShWpkh);
}

fn send_lbtc(signers: &[&Signer], variant: Variant) {
    let (variant, path, closing) = match variant {
        Variant::Wpkh => ("elwpkh", "84h/1h/0h", ""),
        Variant::ShWpkh => ("elsh(wpkh", "49h/1h/0h", ")"),
    };
    let master_node = signers[0].xpub().unwrap();
    let fingerprint = master_node.fingerprint();
    let xpub = signers[0]
        .derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).unwrap())
        .unwrap();

    let slip77_key = generate_slip77();

    // m / purpose' / coin_type' / account' / change / address_index
    let desc_str =
        format!("ct(slip77({slip77_key}),{variant}([{fingerprint}/{path}]{xpub}/1/*){closing})");

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));
}

#[test]
fn emul_send_lbtc_multisig() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    send_lbtc_multisig(jade_init.jade)
}

fn send_lbtc_multisig(mut s1: MutexJade) {
    let s1_xpub = s1.get_mut().unwrap().get_master_xpub().unwrap();
    let s1_fingerprint = s1_xpub.fingerprint();

    let s2 = generate_signer();
    let s2_xpub = s2.xpub();
    let s2_fingerprint = s2_xpub.fingerprint();

    let slip77 = generate_slip77();

    let desc_str = format!(
        "ct(slip77({slip77}),elwsh(multi(2,[{s1_fingerprint}]{s1_xpub}/1/*,[{s2_fingerprint}]{s2_xpub}/1/*)))"
    );
    let wollet_desc: WolletDescriptor = desc_str.parse().unwrap();
    let descriptor: JadeDescriptor = wollet_desc.as_ref().try_into().unwrap();

    s1.get_mut()
        .unwrap()
        .register_multisig(RegisterMultisigParams {
            network: jade::Network::LocaltestLiquid,
            multisig_name: "peppino".to_string(),
            descriptor,
        })
        .unwrap();

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let signers = [&Signer::Jade(&s1), &Signer::Software(s2)];
    let node_address = server.node_getnewaddress();
    wallet.send_btc(&signers, None, Some((node_address, 10_000)));
}
