use crate::init_logging;
use lwk_containers::testcontainers::{clients::Cli, Container};
use lwk_containers::{LedgerEmulator, LEDGER_EMULATOR_PORT};
use lwk_ledger::Ledger;

/// A struct for Ledger testing with emulator
pub struct TestLedgerEmulator<'a> {
    pub ledger: Ledger,
    // Keep the containers so it's not dropped.
    _ledger_emul: Container<'a, LedgerEmulator>,
}

impl<'a> TestLedgerEmulator<'a> {
    /// Ledger with emulator
    pub fn new(docker: &'a Cli) -> Self {
        init_logging();
        let ledger = LedgerEmulator::new().expect("test");
        let container = docker.run(ledger);
        let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT);
        let ledger = Ledger::new(port);
        Self {
            ledger,
            _ledger_emul: container,
        }
    }
}
