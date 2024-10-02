use lwk_common::{singlesig_desc, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_ledger::TestLedgerEmulator;
use lwk_signer::AnySigner;

use crate::test_wollet::{test_client_electrum, TestWollet};

#[test]
fn emul_roundtrip_wpkh() {
    let server = lwk_test_util::setup();
    let docker = Cli::default();
    let ledger = TestLedgerEmulator::new(&docker);
    // TODO
    use elements_miniscript::bitcoin::hashes::Hash;
    let xpub_identifier = elements_miniscript::bitcoin::XKeyIdentifier::all_zeros();
    let signers = &[&AnySigner::Ledger(ledger.ledger, xpub_identifier)];

    let desc_str = singlesig_desc(
        signers[0],
        Singlesig::Wpkh,
        lwk_common::DescriptorBlindingKey::Slip77,
        false,
    )
    .unwrap();
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);

    //let node_address = server.node_getnewaddress();
    //wallet.send_btc(signers, None, Some((node_address, 10_000)));
}
