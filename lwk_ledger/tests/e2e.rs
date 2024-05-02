use bitcoin::bip32::DerivationPath;
use lwk_containers::testcontainers::clients;
use lwk_containers::{LedgerEmulator, LEDGER_EMULATOR_PORT};
use lwk_ledger::*;

#[test]
fn test_ledger_commands() {
    let docker = clients::Cli::default();
    let ledger = LedgerEmulator::new().expect("test");
    let container = docker.run(ledger);
    let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT);
    let client = new(port);
    let (name, version, _flags) = client.get_version().unwrap();
    assert_eq!(version, "2.0.4");
    assert_eq!(name, "Liquid Regtest");
    let fingerprint = client.get_master_fingerprint().unwrap();
    assert_eq!(fingerprint.to_string(), "f5acc2fd");

    let path: DerivationPath = "m/44h/1h/0h".parse().unwrap();
    let xpub = client.get_extended_pubkey(&path, false).unwrap();
    assert_eq!(xpub.to_string(), "tpubDCwYjpDhUdPGP5rS3wgNg13mTrrjBuG8V9VpWbyptX6TRPbNoZVXsoVUSkCjmQ8jJycjuDKBb9eataSymXakTTaGifxR6kmVsfFehH1ZgJT");

    // TODO: uncomment this once something can confirm on the ledger emulator
    //       (it seems that --display headless does not work...)
    /*
    let message = [0u8];
    let (prefix, sig) = client.sign_message(&message, &path).unwrap();
    assert_eq!(prefix, 27+4);
    assert_eq!(sig.to_string(), "TODO");
     * */
}
