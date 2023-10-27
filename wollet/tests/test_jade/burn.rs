use bs_containers::testcontainers::clients::Cli;
use signer::Signer;

use crate::{
    test_jade::init::inner_jade_debug_initialization,
    test_session::{setup, wpkh_desc, TestWollet},
    TEST_MNEMONIC,
};

#[cfg(feature = "serial")]
mod serial {
    use signer::Signer;

    use crate::test_jade::init::serial;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_burn_asset() {
        let jade = serial::unlock();
        let signers = [&Signer::Jade(&jade)];

        super::burn(&signers);
    }
}

#[test]
fn emul_burn_asset() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = [&Signer::Jade(&jade_init.jade)];

    burn(&signers);
}

fn burn(signers: &[&Signer]) {
    let desc_str = wpkh_desc(signers[0]);

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let asset = wallet.fund_asset(&server);

    wallet.burnasset(signers, 1_000, &asset, None);
}
