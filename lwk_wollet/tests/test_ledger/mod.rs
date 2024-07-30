use lwk_common::{singlesig_desc, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_ledger::TestLedgerEmulator;
use lwk_signer::AnySigner;
use lwk_test_util::setup;

use crate::test_wollet::{generate_signer, multisig_desc, TestWollet};

fn emul_roundtrip(singlesig_type: Singlesig) {
    let server = setup(false);
    let docker = Cli::default();
    let ledger = TestLedgerEmulator::new(&docker);
    // TODO
    use elements_miniscript::bitcoin::hashes::Hash;
    let xpub_identifier = elements_miniscript::bitcoin::XKeyIdentifier::all_zeros();
    let signers = &[&AnySigner::Ledger(ledger.ledger, xpub_identifier)];

    let desc_str = singlesig_desc(
        signers[0],
        singlesig_type,
        lwk_common::DescriptorBlindingKey::Slip77,
        false,
    )
    .unwrap();
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    let (asset, _token) = wallet.issueasset(signers, 10, 1, None, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 5, &asset, None);
}

#[test]
fn emul_roundtrip_wpkh() {
    emul_roundtrip(Singlesig::Wpkh);
}

#[test]
fn emul_roundtrip_shwpkh() {
    emul_roundtrip(Singlesig::ShWpkh);
}

#[test]
fn emul_roundtrip_ledger_multisig() {
    let server = setup(false);
    let docker = Cli::default();
    let ledger = TestLedgerEmulator::new(&docker);
    // TODO
    use elements_miniscript::bitcoin::hashes::Hash;
    let xpub_identifier = elements_miniscript::bitcoin::XKeyIdentifier::all_zeros();
    let sw_signer = generate_signer();
    let signers = &[
        &AnySigner::Ledger(ledger.ledger, xpub_identifier),
        &AnySigner::Software(sw_signer),
    ];

    let desc_str = multisig_desc(signers, 2);
    println!("LEOO {}", desc_str);
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let node_address = server.node_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    /*
    let (asset, _token) = wallet.issueasset(signers, 10, 1, None, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 5, &asset, None);
     * */
}
