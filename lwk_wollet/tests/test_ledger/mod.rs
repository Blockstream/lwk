use lwk_common::{singlesig_desc, Singlesig};
use lwk_containers::testcontainers::clients::Cli;
use lwk_ledger::TestLedgerEmulator;
use lwk_signer::AnySigner;
use lwk_test_util::{TestEnv, TestEnvBuilder};

use crate::test_wollet::{generate_signer, multisig_desc, test_client_electrum, TestWollet};

use elements_miniscript::bitcoin::hashes::Hash;

fn roundtrip(
    env: &TestEnv,
    signers: &[&AnySigner],
    variant: Option<lwk_common::Singlesig>,
    threshold: Option<usize>,
) {
    let desc_str = match signers.len() {
        1 => singlesig_desc(
            signers[0],
            variant.unwrap(),
            lwk_common::DescriptorBlindingKey::Slip77,
        )
        .unwrap(),
        _ => multisig_desc(signers, threshold.unwrap()),
    };
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(env);

    let node_address = env.elementsd_getnewaddress();
    wallet.send_btc(signers, None, Some((node_address, 10_000)));

    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":2,\"ticker\":\"TEST\",\"version\":0}";

    let (asset, _token) = wallet.issueasset(signers, 10, 1, Some(contract), None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 5, &asset, None);
}

fn emul_roundtrip_singlesig(variant: Singlesig) {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let ledger = TestLedgerEmulator::new(&docker);
    // TODO
    let xpub_identifier = elements_miniscript::bitcoin::XKeyIdentifier::all_zeros();
    let signers = &[&AnySigner::Ledger(ledger.ledger, xpub_identifier)];
    roundtrip(&env, signers, Some(variant), None);
}

fn emul_roundtrip_multisig(threshold: usize) {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let docker = Cli::default();
    let ledger = TestLedgerEmulator::new(&docker);
    let xpub_identifier = elements_miniscript::bitcoin::XKeyIdentifier::all_zeros();
    let sw_signer = generate_signer();
    let signers = &[
        &AnySigner::Ledger(ledger.ledger, xpub_identifier),
        &AnySigner::Software(sw_signer),
    ];
    roundtrip(&env, signers, None, Some(threshold));
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
