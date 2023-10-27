use bs_containers::testcontainers::clients::Cli;
use jade::{
    get_receive_address::Variant,
    mutex_jade::MutexJade,
    register_multisig::{JadeDescriptor, RegisterMultisigParams},
};
use signer::Signer;
use std::convert::TryInto;
use wollet::WolletDescriptor;

use crate::{
    test_jade::init::inner_jade_debug_initialization,
    test_session::{generate_signer, multisig_desc, setup, singlesig_desc, TestWollet},
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
    let desc_str = singlesig_desc(signers[0], variant);

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
    let s2 = generate_signer();
    let signers = [&Signer::Jade(&s1), &Signer::Software(s2.clone())];
    let threshold = 2;
    let desc_str = multisig_desc(&signers, threshold);
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
    // FIXME: handle s1 in a nicer way
    let signers = [&Signer::Jade(&s1), &Signer::Software(s2)];

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(&signers, None, Some((node_address, 10_000)));
}
