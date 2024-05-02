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

    let path: DerivationPath = "m/44h/1h/0h/0/0".parse().unwrap();
    let message = [3u8; 32];
    let (prefix, sig) = client.sign_message(&message, &path).unwrap();
    assert_eq!(prefix, 27 + 4);
    assert_eq!(
        sig.to_string(),
        "3044022031e78eaebca6af2157bff0ddf9ed40498c8b9d4b184bfb0ff893959beb6d794c022033e0ecd424b5d4a31e946e29f06d2da185c2ae5c7d3d63a71dd6115ec5516464",
    );
    // TODO: verify

    let master_blinding_key = client.get_master_blinding_key().unwrap();
    assert_eq!(
        master_blinding_key.to_string(),
        "0c11648c2c6df4f9dacdb4c8d35d6166d94cea2b9ad37833a82210bb7c9f5fb4"
    );
}
