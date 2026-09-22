use crate::{Ledger, TransportTcp};
use lwk_containers::testcontainers::runners::SyncRunner;
use lwk_containers::testcontainers::Container;
use lwk_containers::{LedgerEmulator, LEDGER_EMULATOR_PORT};

/// A struct for Ledger testing with emulator
pub struct TestLedgerEmulator {
    pub ledger: Ledger<TransportTcp>,
    // Keep the containers so it's not dropped.
    _ledger_emul: Container<LedgerEmulator>,
}

impl TestLedgerEmulator {
    /// Ledger with emulator
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let ledger = LedgerEmulator::new().expect("test");
        let container = ledger.start().unwrap();
        let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT).unwrap();
        let ledger = Ledger::new(port, lwk_common::Network::default_regtest());
        Self {
            ledger,
            _ledger_emul: container,
        }
    }
}
