use lwk_containers::testcontainers::clients;
use lwk_containers::{LedgerEmulator, LEDGER_EMULATOR_PORT};
use lwk_ledger::*;

#[test]
fn test_ledger_version() {
    let docker = clients::Cli::default();
    let ledger = LedgerEmulator::new().expect("test");
    let container = docker.run(ledger);
    let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT);
    let client = new(port);
    let (_name, version, _flags) = client.get_version().unwrap();
    assert_eq!(version, "2.0.4");
}
