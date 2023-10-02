use bitcoin::bip32::ExtendedPubKey;
use bs_containers::{
    jade::{JadeEmulator, EMULATOR_PORT},
    pin_server::{PinServerEmulator, PIN_SERVER_PORT},
};
use ciborium::Value;
use elements::AddressParams;
use jade::{
    protocol::{
        GetReceiveAddressParams, GetXpubParams, HandshakeCompleteParams, HandshakeParams,
        UpdatePinserverParams,
    },
    Jade,
};
use std::{str::FromStr, time::UNIX_EPOCH};
use tempfile::{tempdir, TempDir};
use testcontainers::{
    clients::{self, Cli},
    Container,
};

use crate::pin_server::verify;

const _TEST_MNEMONIC: &str = "fish inner face ginger orchard permit
                             useful method fence kidney chuckle party
                             favorite sunset draw limb science crane
                             oval letter slot invite sadness banana";

#[test]
fn entropy() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.add_entropy(&[1, 2, 3, 4]).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn epoch() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let seconds = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = jade_api.set_epoch(seconds).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn ping() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.ping().unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn version() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.version_info().unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn update_pinserver() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let tempdir = tempdir().unwrap();
    let pin_server = PinServerEmulator::new(&tempdir);
    let pub_key: Vec<u8> = pin_server.pub_key().to_bytes();
    let container = docker.run(pin_server);
    let port = container.get_host_port_ipv4(PIN_SERVER_PORT);
    let url_a = format!("http://127.0.0.1:{}", port);

    let params = UpdatePinserverParams {
        reset_details: false,
        reset_certificate: false,
        url_a,
        url_b: "".to_string(),
        pubkey: Value::Bytes(pub_key),
        certificate: "".into(),
    };
    let result = jade_api.update_pinserver(params).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn jade_initialization() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let result = initialized_jade.jade.version_info().unwrap();
    insta::assert_yaml_snapshot!(result);
    assert!(result.jade_has_pin);
}

#[test]
fn jade_xpub() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let params = GetXpubParams {
        network: jade::Network::TestnetLiquid,
        path: vec![],
    };
    let result = initialized_jade.jade.get_xpub(params).unwrap();
    let xpub_master = ExtendedPubKey::from_str(result.get()).unwrap();
    assert_eq!(xpub_master.depth, 0);
    assert_eq!(xpub_master.network, bitcoin::Network::Testnet);

    let params = GetXpubParams {
        network: jade::Network::TestnetLiquid,
        path: vec![0],
    };
    let result = initialized_jade.jade.get_xpub(params).unwrap();
    let xpub = ExtendedPubKey::from_str(result.get()).unwrap();
    assert_ne!(xpub_master, xpub);
    assert_eq!(xpub.depth, 1);
}

#[test]
fn jade_receive_address() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let params = GetReceiveAddressParams {
        network: jade::Network::LocaltestLiquid,
        variant: "sh(wpkh(k))".into(),
        path: [2147483697, 2147483648, 2147483648, 0, 143].to_vec(),
    };
    let result = initialized_jade.jade.get_receive_address(params).unwrap();
    let address = elements::Address::from_str(result.get()).unwrap();
    assert!(address.blinding_pubkey.is_some());
    assert_eq!(address.params, &AddressParams::ELEMENTS);
}

/// Note underscore prefixed var must be there even if they are not read so that they are not
/// dropped
struct InitializedJade<'a> {
    _pin_server: Container<'a, PinServerEmulator>,
    _jade_emul: Container<'a, JadeEmulator>,
    _tempdir: TempDir,
    jade: Jade,
}

fn inner_jade_initialization(docker: &Cli) -> InitializedJade {
    let jade_container = docker.run(JadeEmulator);
    let port = jade_container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let tempdir = PinServerEmulator::tempdir();
    let pin_server = PinServerEmulator::new(&tempdir);
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
        pubkey: Value::Bytes(pin_server_pub_key.to_bytes()),
        certificate: "".into(),
    };

    let result = jade_api.update_pinserver(params).unwrap();
    assert!(result.get());

    let result = jade_api.auth_user(jade::Network::Liquid).unwrap();
    let start_handshake_url = &result.urls()[0];
    assert_eq!(
        start_handshake_url,
        &format!("{pin_server_url}/start_handshake")
    );

    let resp = ureq::post(start_handshake_url).call().unwrap();
    let params: HandshakeParams = resp.into_json().unwrap();
    verify(&params, &pin_server_pub_key);

    let result = jade_api.handshake_init(params).unwrap();
    let handshake_data = result.data();
    let next_url = &result.urls()[0];
    assert_eq!(next_url, &format!("{pin_server_url}/set_pin"));
    let resp = ureq::post(next_url).send_json(handshake_data).unwrap();
    assert_eq!(resp.status(), 200);
    let params: HandshakeCompleteParams = resp.into_json().unwrap();

    let result = jade_api.handshake_complete(params).unwrap();
    assert!(result.get());

    InitializedJade {
        _pin_server: pin_container,
        _jade_emul: jade_container,
        _tempdir: tempdir,
        jade: jade_api,
    }
}
