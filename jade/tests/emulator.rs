use base64::Engine;
use bs_containers::testcontainers::{
    clients::{self, Cli},
    Container,
};
use bs_containers::{
    jade::{JadeEmulator, EMULATOR_PORT},
    pin_server::{PinServerEmulator, PIN_SERVER_PORT},
};
use elements::{
    bitcoin,
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{ecdsa::Signature, Message, Secp256k1},
    Address, AddressParams,
};
use elements::{
    bitcoin::{bip32::ExtendedPubKey, sign_message::signed_msg_hash},
    hashes::Hash,
};
use elements_miniscript::{
    confidential::Key, ConfidentialDescriptor, DefiniteDescriptorKey, DescriptorPublicKey,
};
use jade::{
    get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant},
    protocol::{JadeState, VersionInfoResult},
    register_multisig::{JadeDescriptor, MultisigSigner, RegisterMultisigParams},
};
use jade::{
    protocol::{
        DebugSetMnemonicParams, GetSignatureParams, GetXpubParams, HandshakeCompleteParams,
        HandshakeParams, SignMessageParams, UpdatePinserverParams,
    },
    Jade,
};
use std::{str::FromStr, time::UNIX_EPOCH, vec};
use tempfile::{tempdir, TempDir};

use crate::pin_server::verify;

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn entropy() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);

    let result = jade_api.add_entropy(&[1, 2, 3, 4]).unwrap();
    assert!(result);
}

#[test]
fn debug_set_mnemonic() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);

    let result = initialized_jade.jade.version_info().unwrap();
    assert_eq!(result, mock_version_info());
}

#[test]
fn epoch() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);

    let seconds = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = jade_api.set_epoch(seconds).unwrap();
    assert!(result);
}

#[test]
fn ping() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);

    let result = jade_api.ping().unwrap();
    assert_eq!(result, 0);
}

#[test]
fn version() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);

    let result = jade_api.version_info().unwrap();
    let mut expected = mock_version_info();
    expected.jade_state = JadeState::Uninit;
    assert_eq!(result, expected);
}

#[test]
fn update_pinserver() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);

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
        pubkey: pub_key,
        certificate: "".into(),
    };
    let result = jade_api.update_pinserver(params).unwrap();
    assert!(result);
}

#[test]
fn jade_initialization_with_pin_server() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let result = initialized_jade.jade.version_info().unwrap();
    let mut expected = mock_version_info();
    expected.jade_has_pin = true;
    assert_eq!(result, expected);
}

#[test]
fn jade_init_logout_unlock() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let jade = &mut initialized_jade.jade;
    assert!(jade.logout().unwrap());
    jade.unlock().unwrap();
}

#[test]
fn jade_xpub() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let jade = &mut initialized_jade.jade;
    let xpub_master = jade.get_master_xpub().unwrap();
    assert_eq!(xpub_master.depth, 0);
    assert_eq!(xpub_master.network, bitcoin::Network::Testnet);

    let params = GetXpubParams {
        network: jade::Network::LocaltestLiquid,
        path: vec![0],
    };
    let xpub = jade.get_xpub(params).unwrap();
    assert_ne!(xpub_master, xpub);
    assert_eq!(xpub.depth, 1);
    assert_eq!(jade.fingerprint().unwrap().as_bytes(), &[115, 197, 218, 10]);
}

#[test]
fn jade_receive_address() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let params = GetReceiveAddressParams {
        network: jade::Network::LocaltestLiquid,
        address: SingleOrMulti::Single {
            variant: Variant::ShWpkh,
            path: vec![2147483697, 2147483648, 2147483648, 0, 143],
        },
    };
    let result = initialized_jade.jade.get_receive_address(params).unwrap();
    let address = elements::Address::from_str(&result).unwrap();
    assert!(address.blinding_pubkey.is_some());
    assert_eq!(address.params, &AddressParams::ELEMENTS);
}

#[test]
fn jade_register_multisig() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let jade = &mut initialized_jade.jade;

    let jade_master_xpub = jade.get_master_xpub().unwrap();

    let params = GetXpubParams {
        network: jade::Network::LocaltestLiquid,
        path: vec![0, 1],
    };
    let jade_xpub = jade.get_xpub(params).unwrap();

    let signers = vec![
        MultisigSigner {
            fingerprint: vec![2u8; 4],
            derivation: vec![],
            xpub: "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG".parse().unwrap(),
            path: vec![],
        },
        MultisigSigner {
            fingerprint: jade_master_xpub.fingerprint().to_bytes().to_vec(),
            derivation: vec![0,1],
            xpub: jade_xpub,
            path: vec![],
        }
    ];

    let params = RegisterMultisigParams {
        network: jade::Network::LocaltestLiquid,
        multisig_name: "finney-satoshi".to_string(),
        descriptor: JadeDescriptor {
            variant: "wsh(multi(k))".to_string(),
            sorted: false,
            threshold: 2,
            master_blinding_key: hex::decode(
                "afacc503637e85da661ca1706c4ea147f1407868c48d8f92dd339ac272293cdc",
            )
            .unwrap(),
            signers,
        },
    };
    let result = jade.register_multisig(params).unwrap();
    assert!(result)
}

#[test]
fn jade_register_multisig_check_address() {
    let docker = clients::Cli::default();

    let network = jade::Network::LocaltestLiquid;
    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let jade = &mut initialized_jade.jade;

    let multisig_name = "you_and_me".to_string();
    let jade_master_xpub = jade.get_master_xpub().unwrap();
    let other_signer: ExtendedPubKey= "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG".parse().unwrap();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

    let desc =
        format!("ct(slip77({slip77_key}),elwsh(multi(2,{jade_master_xpub}/*,{other_signer}/*)))");

    let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc.parse().unwrap();
    let jade_desc: JadeDescriptor = (&desc).try_into().unwrap();
    jade.register_multisig(RegisterMultisigParams {
        network,
        multisig_name: multisig_name.clone(),
        descriptor: jade_desc,
    })
    .unwrap();

    let result = jade.get_registered_multisigs().unwrap();
    assert_eq!(result.len(), 1);
    result.get(&multisig_name).unwrap();

    let result = jade
        .get_receive_address(GetReceiveAddressParams {
            network,
            address: SingleOrMulti::Multi {
                multisig_name,
                paths: vec![vec![0], vec![0]],
            },
        })
        .unwrap();
    let address_jade: Address = result.parse().unwrap();

    // copied from wollet derive_address
    let derived_non_conf = desc.descriptor.at_derivation_index(0).unwrap();
    let derived_conf = ConfidentialDescriptor::<DefiniteDescriptorKey> {
        key: match desc.key {
            Key::Slip77(x) => Key::Slip77(x),
            _ => panic!("wrong master blinding key type"),
        },
        descriptor: derived_non_conf,
    };
    let address_desc = derived_conf
        .address(&Secp256k1::new(), &AddressParams::ELEMENTS)
        .unwrap();

    assert_eq!(address_desc, address_jade);
}

#[test]
fn jade_sign_message() {
    // TODO create anti exfil commitments
    // The following are taken from jade tests, even though they may be random if we are not verifying.
    // To create the commitment jade use wally_ae_host_commit_from_bytes, rust-secp at the moment
    // doesn't expose exfil methods
    let ae_host_commitment =
        hex::decode("7b61fad27ce2d95abca09f76bd7226e50212a8542f3ca274ee546cec4bc5c3bb").unwrap();
    let ae_host_entropy =
        hex::decode("3f5540b9336af9bdd50a5b7f69fc2045a12e3b3e0740f7461902d882bf8a8820").unwrap();
    let docker = clients::Cli::default();
    let message = "Hello world!";
    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let params = SignMessageParams {
        message: message.to_string(),
        path: vec![0],
        ae_host_commitment,
    };
    let _signer_commitment: Vec<u8> = initialized_jade.jade.sign_message(params).unwrap().to_vec();

    let params = GetSignatureParams { ae_host_entropy };
    let signature = initialized_jade.jade.get_signature_for_msg(params).unwrap();
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .unwrap();

    let params = GetXpubParams {
        network: jade::Network::LocaltestLiquid,
        path: vec![0],
    };
    let xpub = initialized_jade.jade.get_xpub(params).unwrap();
    let msg_hash = signed_msg_hash(message);
    let message = Message::from_slice(msg_hash.as_byte_array()).unwrap();
    let signature = Signature::from_compact(&signature_bytes).unwrap();

    assert!(Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &xpub.public_key)
        .is_ok());

    //TODO verify anti-exfil
}

#[test]
fn jade_sign_liquid_tx() {
    let docker = clients::Cli::default();
    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let pset_base64 = include_str!("../test_data/pset_to_be_signed.base64");
    let mut pset: PartiallySignedTransaction = pset_base64.parse().unwrap();
    assert_eq!(pset.outputs().len(), 3);

    initialized_jade.jade.sign_pset(&mut pset).unwrap();
}

/// Note underscore prefixed var must be there even if they are not read so that they are not
/// dropped
struct InitializedJade<'a> {
    _pin_server: Option<Container<'a, PinServerEmulator>>,
    _jade_emul: Container<'a, JadeEmulator>,
    _tempdir: Option<TempDir>,
    jade: Jade,
}

fn inner_jade_initialization(docker: &Cli) -> InitializedJade {
    let jade_container = docker.run(JadeEmulator);
    let port = jade_container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::Liquid);

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
        pubkey: pin_server_pub_key.to_bytes(),
        certificate: "".into(),
    };

    let result = jade_api.update_pinserver(params).unwrap();
    assert!(result);

    let result = jade_api.auth_user().unwrap();
    let start_handshake_url = &result.urls()[0];
    assert_eq!(
        start_handshake_url,
        &format!("{pin_server_url}/start_handshake")
    );

    let resp = minreq::post(start_handshake_url).send().unwrap();
    let params: HandshakeParams = serde_json::from_slice(resp.as_bytes()).unwrap();
    verify(&params, &pin_server_pub_key);

    let result = jade_api.handshake_init(params).unwrap();
    let handshake_data = result.data();
    let next_url = &result.urls()[0];
    assert_eq!(next_url, &format!("{pin_server_url}/set_pin"));
    let data = serde_json::to_vec(&handshake_data).unwrap();
    let resp = minreq::post(next_url).with_body(data).send().unwrap();
    assert_eq!(resp.status_code, 200);
    let params: HandshakeCompleteParams = serde_json::from_slice(resp.as_bytes()).unwrap();

    let result = jade_api.handshake_complete(params).unwrap();
    assert!(result);

    InitializedJade {
        _pin_server: Some(pin_container),
        _jade_emul: jade_container,
        _tempdir: Some(tempdir),
        jade: jade_api,
    }
}

fn inner_jade_debug_initialization(docker: &Cli) -> InitializedJade {
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);
    let params = DebugSetMnemonicParams {
        mnemonic: TEST_MNEMONIC.to_string(),
        passphrase: None,
        temporary_wallet: false,
    };
    let result = jade_api.debug_set_mnemonic(params).unwrap();
    assert!(result);

    InitializedJade {
        _pin_server: None,
        _jade_emul: container,
        _tempdir: None,
        jade: jade_api,
    }
}

fn mock_version_info() -> VersionInfoResult {
    VersionInfoResult {
        jade_version: "1".to_string(),
        jade_ota_max_chunk: 4096,
        jade_config: "NORADIO".to_string(),
        board_type: "DEV".to_string(),
        jade_features: "DEV".to_string(),
        idf_version: "v5.0.2".to_string(),
        chip_features: "32000000".to_string(),
        efusemac: "000000000000".to_string(),
        battery_status: 0,
        jade_state: JadeState::Ready,
        jade_networks: "ALL".to_string(),
        jade_has_pin: false,
    }
}
