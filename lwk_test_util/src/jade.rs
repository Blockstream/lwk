use crate::init_logging;
use elements::{
    bitcoin::PublicKey,
    hashes::{hex::FromHex, sha256, Hash},
    secp256k1_zkp::{ecdsa::Signature, Message, Secp256k1},
};
use lwk_containers::testcontainers::{clients::Cli, Container};
use lwk_containers::{JadeEmulator, PinServer, EMULATOR_PORT, PIN_SERVER_PORT};
use lwk_jade::{
    mutex_jade::MutexJade,
    protocol::{DebugSetMnemonicParams, HandshakeInitParams, UpdatePinserverParams},
    Jade, Network,
};
use tempfile::TempDir;

pub fn pin_server_verify(params: &HandshakeInitParams, pin_server_pub_key: &PublicKey) {
    let ske_bytes = Vec::<u8>::from_hex(&params.ske).unwrap();
    let ske_hash = sha256::Hash::hash(&ske_bytes);

    let signature_bytes = Vec::<u8>::from_hex(&params.sig).unwrap();
    let signature = Signature::from_compact(&signature_bytes).unwrap();

    let message = Message::from_digest_slice(&ske_hash[..]).unwrap();

    let verify = Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &pin_server_pub_key.inner)
        .is_ok();
    assert!(verify);
}

/// A struct for Jade testing with emulator
pub struct TestJadeEmulator<'a> {
    pub jade: Jade,
    // Keep the containers and temp dir so they are not dropped.
    _jade_emul: Container<'a, JadeEmulator>,
    _pin_server: Option<Container<'a, PinServer>>,
    _pin_server_dir: Option<TempDir>,
}

impl<'a> TestJadeEmulator<'a> {
    /// Jade with emulator
    pub fn new(docker: &'a Cli) -> Self {
        init_logging();
        let container = docker.run(JadeEmulator);
        let port = container.get_host_port_ipv4(EMULATOR_PORT);
        let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let network = Network::LocaltestLiquid;
        let jade = Jade::new(stream.into(), network);
        Self {
            jade,
            _jade_emul: container,
            _pin_server: None,
            _pin_server_dir: None,
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

    /// Jade with emulator and dedicated pin server
    pub fn new_with_pin(docker: &'a Cli) -> Self {
        let mut test_jade_emul = Self::new(docker);

        let tempdir = PinServer::tempdir().unwrap();
        let pin_server = PinServer::new(&tempdir).unwrap();
        let pin_server_pub_key = *pin_server.pub_key();
        assert_eq!(pin_server_pub_key.to_bytes().len(), 33);
        let pin_container = docker.run(pin_server);
        let port = pin_container.get_host_port_ipv4(PIN_SERVER_PORT);
        let pin_server_url = format!("http://127.0.0.1:{}", port);

        let params = UpdatePinserverParams {
            reset_details: false,
            reset_certificate: false,
            url_a: pin_server_url.clone(),
            url_b: "".to_string(),
            pubkey: pin_server_pub_key.to_bytes(),
            certificate: "".into(),
        };

        let result = test_jade_emul.jade.update_pinserver(params).unwrap();
        assert!(result);

        test_jade_emul.jade.unlock().unwrap();

        test_jade_emul._pin_server = Some(pin_container);
        test_jade_emul._pin_server_dir = Some(tempdir);
        test_jade_emul
    }
}

/// A struct for MutexJade testing with emulator
pub struct TestMutexJadeEmulator<'a> {
    pub jade: MutexJade,
    // Keep the containers and temp dir so they are not dropped.
    _jade_emul: Container<'a, JadeEmulator>,
    _pin_server: Option<Container<'a, PinServer>>,
    _pin_server_dir: Option<TempDir>,
}

impl<'a> TestMutexJadeEmulator<'a> {
    /// MutexJade with emulator
    pub fn new(test_jade_emul: TestJadeEmulator<'a>) -> Self {
        let TestJadeEmulator {
            jade,
            _jade_emul,
            _pin_server,
            _pin_server_dir,
        } = test_jade_emul;
        let jade = MutexJade::new(jade);
        Self {
            jade,
            _jade_emul,
            _pin_server,
            _pin_server_dir,
        }
    }
}
