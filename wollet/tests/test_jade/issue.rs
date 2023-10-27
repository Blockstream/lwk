use bs_containers::testcontainers::clients::Cli;
use signer::Signer;

use crate::{
    test_jade::init::inner_jade_debug_initialization,
    test_session::{setup, wpkh_desc, TestWollet},
    TEST_MNEMONIC,
};

#[cfg(feature = "serial")]
mod serial {
    use crate::test_jade::init::serial;
    use signer::Signer;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_issue_asset() {
        let jade = serial::unlock();
        let signers = [&Signer::Jade(&jade)];

        super::issue_asset_contract(&signers);
    }
}

#[test]
fn emul_issue_asset() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = [&Signer::Jade(&jade_init.jade)];

    issue_asset_contract(&signers);
}

fn issue_asset_contract(signers: &[&Signer]) {
    let desc_str = wpkh_desc(signers[0]);

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":2,\"ticker\":\"TEST\",\"version\":0}";

    let (asset, _token) = wallet.issueasset(signers, 1_000, 1, contract, None);
    dbg!(asset); // f56d514
}
