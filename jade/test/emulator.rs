use base64::Engine;
use bs_containers::{
    jade::{JadeEmulator, EMULATOR_PORT},
    pin_server::{PinServerEmulator, PIN_SERVER_PORT},
};
use elements::{
    bitcoin,
    encode::serialize,
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{ecdsa::Signature, Message},
    AddressParams,
};
use elements::{
    bitcoin::{bip32::ExtendedPubKey, sign_message::signed_msg_hash},
    hashes::Hash,
};
use jade::{
    protocol::{
        DebugSetMnemonicParams, GetReceiveAddressParams, GetSignatureParams, GetXpubParams,
        HandshakeCompleteParams, HandshakeParams, SignMessageParams, UpdatePinserverParams,
    },
    sign_liquid_tx::{Commitment, SignLiquidTxParams},
    Jade,
};
use std::{str::FromStr, time::UNIX_EPOCH, vec};
use tempfile::{tempdir, TempDir};
use testcontainers::{
    clients::{self, Cli},
    Container,
};

use crate::pin_server::verify;

const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

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
fn debug_set_mnemonic() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);

    let result = initialized_jade.jade.version_info().unwrap();
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
        pubkey: pub_key,
        certificate: "".into(),
    };
    let result = jade_api.update_pinserver(params).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn jade_initialization_with_pin_server() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_initialization(&docker);
    let result = initialized_jade.jade.version_info().unwrap();
    insta::assert_yaml_snapshot!(result);
    assert!(result.jade_has_pin);
}

#[test]
fn jade_xpub() {
    let docker = clients::Cli::default();

    let mut initialized_jade = inner_jade_debug_initialization(&docker);
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

    let mut initialized_jade = inner_jade_debug_initialization(&docker);
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
    let _signer_commitment: Vec<u8> = initialized_jade.jade.sign_message(params).unwrap().into();

    let params = GetSignatureParams { ae_host_entropy };
    let signature = initialized_jade.jade.get_signature(params).unwrap();
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature.get())
        .unwrap();

    let params = GetXpubParams {
        network: jade::Network::TestnetLiquid,
        path: vec![0],
    };
    let result = initialized_jade.jade.get_xpub(params).unwrap();
    let xpub = ExtendedPubKey::from_str(result.get()).unwrap();
    let msg_hash = signed_msg_hash(message);
    let message = Message::from_slice(msg_hash.as_byte_array()).unwrap();
    let signature = Signature::from_compact(&signature_bytes).unwrap();

    assert!(elements::secp256k1_zkp::Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &xpub.public_key)
        .is_ok());

    //TODO verify anti-exfil
}

#[test]
fn jade_sign_liquid_tx() {
    let docker = clients::Cli::default();
    let mut initialized_jade = inner_jade_debug_initialization(&docker);
    let pset_base64 = include_str!("../test_data/pset_to_be_signed.base64");
    let pset: PartiallySignedTransaction = pset_base64.parse().unwrap();
    let tx = pset.extract_tx().unwrap();
    let txn = serialize(&tx);

    assert_eq!(tx.output.len(), 3);
    assert_eq!(pset.outputs().len(), 3);

    let mut trusted_commitments = vec![];
    for output in pset.outputs().iter() {
        let mut asset_id = serialize(&output.asset.unwrap());
        asset_id.reverse(); // Jade want it reversed
        let trusted_commitment = if output.script_pubkey.is_empty() {
            // fee output
            None
        } else {
            Some(Commitment {
                asset_blind_proof: output.blind_asset_proof.as_ref().unwrap().serialize(),
                asset_generator: output.asset_comm.unwrap().serialize().to_vec(),
                asset_id,
                blinding_key: output.blinding_key.unwrap().to_bytes(),
                value: output.amount.unwrap(),
                value_commitment: output.amount_comm.unwrap().serialize().to_vec(),
                value_blind_proof: output.blind_value_proof.as_ref().unwrap().serialize(),
            })
        };
        trusted_commitments.push(trusted_commitment);
    }
    assert_eq!(trusted_commitments.len(), 3);

    let params = SignLiquidTxParams {
        network: jade::Network::LocaltestLiquid,
        txn,
        num_inputs: tx.input.len() as u32,
        use_ae_signatures: false,
        change: vec![None, None, None],
        asset_info: vec![],
        trusted_commitments,
        additional_info: None,
    };
    // println!("{:#?}", params);
    let sign_response = initialized_jade.jade.sign_liquid_tx(params).unwrap().get();
    assert!(sign_response);
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
        pubkey: pin_server_pub_key.to_bytes(),
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
    let mut jade_api = Jade::new(stream.into());
    let params = DebugSetMnemonicParams {
        mnemonic: TEST_MNEMONIC.to_string(),
        passphrase: None,
        temporary_wallet: false,
    };
    let result = jade_api.debug_set_mnemonic(params).unwrap();
    assert!(result.get());

    InitializedJade {
        _pin_server: None,
        _jade_emul: container,
        _tempdir: None,
        jade: jade_api,
    }
}
