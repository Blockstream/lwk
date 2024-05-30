use lwk_common::{singlesig_desc, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_ledger::TestLedgerEmulator;
use lwk_signer::AnySigner;
use lwk_test_util::setup;

use crate::test_wollet::TestWollet;

#[test]
fn emul_roundtrip_wpkh() {
    let server = setup(false);
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
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));
}
