#![allow(clippy::unwrap_used)]

use crate::{
    protocol::{DebugSetMnemonicParams, UpdatePinserverParams},
    Jade, Network,
};
use lwk_containers::testcontainers::runners::SyncRunner;
use lwk_containers::testcontainers::Container;
use lwk_containers::{JadeEmulator, PinServer, EMULATOR_PORT, PIN_SERVER_PORT};
use tempfile::TempDir;

/// A struct for Jade testing with emulator
pub struct TestJadeEmulator {
    pub jade: Jade,
    // Keep the containers and temp dir so they are not dropped.
    _jade_emul: Container<JadeEmulator>,
    _pin_server: Option<Container<PinServer>>,
    _pin_server_dir: Option<TempDir>,
}

impl TestJadeEmulator {
    /// Jade with emulator and a local PIN server
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let container = JadeEmulator::new().start().unwrap();
        let port = container.get_host_port_ipv4(EMULATOR_PORT).unwrap();
        let stream = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let network = Network::default_regtest();
        let jade = Jade::new(stream.into(), network);

        let tempdir = PinServer::tempdir().unwrap();
        let pin_server = PinServer::new(&tempdir).unwrap();
        let pin_server_pub_key = *pin_server.pub_key();
        assert_eq!(pin_server_pub_key.to_bytes().len(), 33);
        let pin_container = pin_server.start().unwrap();
        let pin_port = pin_container.get_host_port_ipv4(PIN_SERVER_PORT).unwrap();
        let pin_server_url = format!("http://127.0.0.1:{pin_port}");

        let params = UpdatePinserverParams {
            reset_details: false,
            reset_certificate: false,
            url_a: pin_server_url.clone(),
            url_b: "".to_string(),
            pubkey: pin_server_pub_key.to_bytes(),
            certificate: "".into(),
        };

        let result = jade.update_pinserver(params).unwrap();
        assert!(result);

        jade.unlock().unwrap();

        Self {
            jade,
            _jade_emul: container,
            _pin_server: Some(pin_container),
            _pin_server_dir: Some(tempdir),
        }
    }

    /// Set a mnemonic
    pub fn set_debug_mnemonic(&mut self, mnemonic: &str) {
        let params = DebugSetMnemonicParams {
            mnemonic: mnemonic.to_string(),
            passphrase: None,
            temporary_wallet: false,
        };
        let result = self.jade.debug_set_mnemonic(params).unwrap();
        assert!(result);
    }

    /// Alias for [`Self::new`], retained for compatibility.
    pub fn new_with_pin() -> Self {
        Self::new()
    }

    /// Get the host port for the emulator
    pub fn emulator_port(&self) -> u16 {
        self._jade_emul.get_host_port_ipv4(EMULATOR_PORT).unwrap()
    }

    /// Releases the TCP connection to the Jade emulator, returning a guard that keeps the
    /// containers alive.
    pub fn release_connection(self) -> TestJadeEmulatorGuard {
        TestJadeEmulatorGuard {
            _jade_emul: self._jade_emul,
            _pin_server: self._pin_server,
            _pin_server_dir: self._pin_server_dir,
        }
    }
}

/// A guard that holds the emulator and PIN server containers to keep them alive
/// after releasing the client connection.
pub struct TestJadeEmulatorGuard {
    _jade_emul: Container<JadeEmulator>,
    _pin_server: Option<Container<PinServer>>,
    _pin_server_dir: Option<TempDir>,
}
