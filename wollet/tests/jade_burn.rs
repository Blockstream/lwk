use bs_containers::testcontainers::clients::Cli;
use elements::bitcoin::bip32::DerivationPath;
use signer::Signer;
use std::str::FromStr;

use crate::{
    init_jade::inner_jade_debug_initialization,
    test_session::{generate_slip77, setup, TestWollet},
    TEST_MNEMONIC,
};

#[cfg(feature = "serial")]
mod serial {
    use signer::Signer;

    use crate::init_jade::serial::init_and_unlock_serial_jade;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_burn() {
        let mut jade = init_and_unlock_serial_jade();
        let signers = [&Signer::Jade(&jade)];

        super::burn(&signers);

        jade.get_mut().unwrap().logout().unwrap();
    }
}

#[test]
fn emul_burn() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = [&Signer::Jade(&jade_init.jade)];

    burn(&signers);
}

fn burn(signers: &[&Signer]) {
    let path = "84h/1h/0h";
    let master_node = signers[0].xpub().unwrap();
    let fingerprint = master_node.fingerprint();
    let xpub = signers[0]
        .derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).unwrap())
        .unwrap();

    let slip77_key = generate_slip77();

    // m / purpose' / coin_type' / account' / change / address_index
    let desc_str = format!("ct(slip77({slip77_key}),elwpkh([{fingerprint}/{path}]{xpub}/1/*))");

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let asset = wallet.fund_asset(&server);

    wallet.burnasset(signers, 1_000, &asset, None);
}
