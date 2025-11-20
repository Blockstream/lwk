use base64::Engine;
use elements::{
    bitcoin::{self, bip32::Fingerprint, bip32::Xpub, sign_message::signed_msg_hash},
    hashes::Hash,
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{ecdsa::Signature, Message, Secp256k1},
    Address, AddressParams,
};
use elements_miniscript::{
    confidential::Key, ConfidentialDescriptor, DefiniteDescriptorKey, DescriptorPublicKey,
};
use lwk_containers::{
    testcontainers::clients::{self},
    PinServer, PIN_SERVER_PORT,
};
use lwk_jade::{
    get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant},
    protocol::{
        GetMasterBlindingKeyParams, GetSignatureParams, GetXpubParams, JadeState,
        SignMessageParams, UpdatePinserverParams, VersionInfoResult,
    },
    register_multisig::{
        GetRegisteredMultisigParams, JadeDescriptor, MultisigSigner, RegisterMultisigParams,
    },
    TestJadeEmulator,
};
use lwk_test_util::TEST_MNEMONIC;
use std::{str::FromStr, time::UNIX_EPOCH, vec};

#[test]
fn entropy() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new(&docker);

    let result = jade.jade.add_entropy([1, 2, 3, 4].to_vec()).unwrap();
    assert!(result);
}

#[test]
fn debug_set_mnemonic() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let result = jade.jade.version_info().unwrap();
    assert_eq!(result, mock_version_info());
}

#[test]
fn epoch() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new(&docker);

    let seconds = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = jade.jade.set_epoch(seconds).unwrap();
    assert!(result);
}

#[test]
fn ping() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new(&docker);

    let result = jade.jade.ping().unwrap();
    assert_eq!(result, 0);
}

#[test]
fn version() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new(&docker);

    let result = jade.jade.version_info().unwrap();
    let mut expected = mock_version_info();
    expected.jade_state = JadeState::Uninit;
    assert_eq!(result, expected);
}

#[test]
fn update_pinserver() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new(&docker);

    let tempdir = tempfile::tempdir().unwrap();
    let pin_server = PinServer::new(&tempdir).unwrap();
    let pub_key: Vec<u8> = pin_server.pub_key().to_bytes();
    let container = docker.run(pin_server);
    let port = container.get_host_port_ipv4(PIN_SERVER_PORT);
    let url_a = format!("http://127.0.0.1:{port}");

    let params = UpdatePinserverParams {
        reset_details: false,
        reset_certificate: false,
        url_a,
        url_b: "".to_string(),
        pubkey: pub_key,
        certificate: "".into(),
    };
    let result = jade.jade.update_pinserver(params).unwrap();
    assert!(result);
}

#[test]
fn jade_initialization_with_pin_server() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new_with_pin(&docker);

    let result = jade.jade.version_info().unwrap();
    let mut expected = mock_version_info();
    expected.jade_has_pin = true;
    assert_eq!(result, expected);
}

#[test]
fn jade_init_logout_unlock() {
    let docker = clients::Cli::default();
    let jade = TestJadeEmulator::new_with_pin(&docker);

    assert!(jade.jade.logout().unwrap());
    jade.jade.unlock().unwrap();
}

#[test]
fn jade_xpub() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let xpub_master = jade.jade.get_master_xpub().unwrap();
    assert_eq!(xpub_master.depth, 0);
    assert_eq!(xpub_master.network, bitcoin::NetworkKind::Test);

    let params = GetXpubParams {
        network: lwk_common::Network::LocaltestLiquid,
        path: vec![0],
    };
    let xpub = jade.jade.get_cached_xpub(params).unwrap();
    assert_ne!(xpub_master, xpub);
    assert_eq!(xpub.depth, 1);
    assert_eq!(
        jade.jade.fingerprint().unwrap().as_bytes(),
        &[115, 197, 218, 10]
    );
}

#[test]
fn jade_receive_address() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let params = GetReceiveAddressParams {
        network: lwk_common::Network::LocaltestLiquid,
        address: SingleOrMulti::Single {
            variant: Variant::ShWpkh,
            path: vec![2147483697, 2147483648, 2147483648, 0, 143],
        },
    };
    let result = jade.jade.get_receive_address(params).unwrap();
    let address = elements::Address::from_str(&result).unwrap();
    assert!(address.blinding_pubkey.is_some());
    assert_eq!(address.params, &AddressParams::ELEMENTS);
}

#[test]
fn jade_register_multisig() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let jade_master_xpub = jade.jade.get_master_xpub().unwrap();

    let params = GetXpubParams {
        network: lwk_common::Network::LocaltestLiquid,
        path: vec![0, 1],
    };
    let jade_xpub = jade.jade.get_cached_xpub(params).unwrap();

    let signers = vec![
        MultisigSigner {
            fingerprint: Fingerprint::from([2u8; 4]),
            derivation: vec![],
            xpub: "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG".parse().unwrap(),
            path: vec![],
        },
        MultisigSigner {
            fingerprint: jade_master_xpub.fingerprint(),
            derivation: vec![0,1],
            xpub: jade_xpub,
            path: vec![],
        }
    ];

    let params = RegisterMultisigParams {
        network: lwk_common::Network::LocaltestLiquid,
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
    let result = jade.jade.register_multisig(params.clone()).unwrap();
    assert!(result)
}

#[test]
#[ignore = "this test is a bit slow"]
fn jade_max_multisigs() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let xpub_params = GetXpubParams {
        network: lwk_common::Network::LocaltestLiquid,
        path: vec![0, 1],
    };
    let jade_signer = MultisigSigner {
        fingerprint: jade.jade.fingerprint().unwrap(),
        derivation: vec![0, 1],
        xpub: jade.jade.get_cached_xpub(xpub_params).unwrap(),
        path: vec![],
    };

    fn _params(jade_signer: &MultisigSigner, index: u32) -> RegisterMultisigParams {
        let s = (0..32)
            .map(|_| format!("{index:02}"))
            .collect::<Vec<_>>()
            .join("");
        RegisterMultisigParams {
            network: lwk_common::Network::LocaltestLiquid,
            multisig_name: index.to_string(),
            descriptor: JadeDescriptor {
                variant: "wsh(multi(k))".to_string(),
                sorted: false,
                threshold: 2,
                master_blinding_key: hex::decode(s).unwrap(),
                signers: vec![
                    MultisigSigner {
                        fingerprint: Fingerprint::from([2u8; 4]),
                        derivation: vec![],
                        xpub: "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG".parse().unwrap(),
                        path: vec![],
                    },
                    jade_signer.clone(),
                ],
            },
        }
    }

    // Register 16 multisig wallets
    for index in 0..16 {
        assert!(jade
            .jade
            .register_multisig(_params(&jade_signer, index))
            .unwrap());
    }
    assert_eq!(jade.jade.get_registered_multisigs().unwrap().len(), 16);

    let err = jade
        .jade
        .register_multisig(_params(&jade_signer, 16))
        .unwrap_err();
    assert!(format!("{err:?}").contains("Already have maximum number of multisig wallets"));
}

#[test]
fn jade_register_multisig_check_address() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let multisig_name = "you_and_me".to_string();
    let jade_master_xpub = jade.jade.get_master_xpub().unwrap();
    let other_signer: Xpub= "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG".parse().unwrap();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

    let desc =
        format!("ct(slip77({slip77_key}),elwsh(multi(2,{jade_master_xpub}/*,{other_signer}/*)))");

    let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc.parse().unwrap();
    let jade_desc: JadeDescriptor = (&desc).try_into().unwrap();
    let network = lwk_common::Network::LocaltestLiquid;
    jade.jade
        .register_multisig(RegisterMultisigParams {
            network,
            multisig_name: multisig_name.clone(),
            descriptor: jade_desc.clone(),
        })
        .unwrap();

    let result = jade.jade.get_registered_multisigs().unwrap();
    assert_eq!(result.len(), 1);
    result.get(&multisig_name).unwrap();

    let result = jade
        .jade
        .get_receive_address(GetReceiveAddressParams {
            network,
            address: SingleOrMulti::Multi {
                multisig_name: multisig_name.clone(),
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

    let result = jade
        .jade
        .get_registered_multisig(GetRegisteredMultisigParams {
            multisig_name: multisig_name.clone(),
        })
        .unwrap();
    assert_eq!(result.multisig_name, multisig_name);
    assert_eq!(result.descriptor, jade_desc);
}

#[test]
fn jade_sign_message() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    // TODO create anti exfil commitments
    // The following are taken from jade tests, even though they may be random if we are not verifying.
    // To create the commitment jade use wally_ae_host_commit_from_bytes, rust-secp at the moment
    // doesn't expose exfil methods
    let ae_host_commitment =
        hex::decode("7b61fad27ce2d95abca09f76bd7226e50212a8542f3ca274ee546cec4bc5c3bb").unwrap();
    let ae_host_entropy =
        hex::decode("3f5540b9336af9bdd50a5b7f69fc2045a12e3b3e0740f7461902d882bf8a8820").unwrap();
    let message = "Hello world!";
    let params = SignMessageParams {
        message: message.to_string(),
        path: vec![0],
        ae_host_commitment,
    };
    let _signer_commitment: Vec<u8> = jade.jade.sign_message_inner(params).unwrap().to_vec();

    let params = GetSignatureParams { ae_host_entropy };
    let signature = jade.jade.get_signature_for_msg(params).unwrap();
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .unwrap();

    let params = GetXpubParams {
        network: lwk_common::Network::LocaltestLiquid,
        path: vec![0],
    };
    let xpub = jade.jade.get_cached_xpub(params).unwrap();
    let msg_hash = signed_msg_hash(message);
    let message = Message::from_digest_slice(msg_hash.as_byte_array()).unwrap();
    let signature = Signature::from_compact(&signature_bytes).unwrap();

    assert!(Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &xpub.public_key)
        .is_ok());

    //TODO verify anti-exfil
}

#[test]
fn jade_sign_liquid_tx() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let pset_base64 = include_str!("../test_data/pset_to_be_signed.base64");
    let mut pset: PartiallySignedTransaction = pset_base64.parse().unwrap();
    assert_eq!(pset.outputs().len(), 3);

    jade.jade.sign(&mut pset).unwrap();
}

#[test]
fn jade_get_master_blinding_key() {
    let docker = clients::Cli::default();
    let mut jade = TestJadeEmulator::new(&docker);
    jade.set_debug_mnemonic(TEST_MNEMONIC);

    let params = GetMasterBlindingKeyParams {
        only_if_silent: false,
    };
    let result = jade.jade.get_master_blinding_key(params).unwrap();
    assert_eq!(hex::encode(result), lwk_test_util::TEST_MNEMONIC_SLIP77);
}

#[cfg(feature = "asyncr")]
#[tokio::test]
async fn async_ping() {
    lwk_test_util::init_logging();

    let docker = clients::Cli::default();

    let container = docker.run(lwk_containers::JadeEmulator);
    let port = container.get_host_port_ipv4(lwk_containers::EMULATOR_PORT);
    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let network = lwk_common::Network::LocaltestLiquid;
    let jade = lwk_jade::asyncr::Jade::new_tcp(stream, network);
    let result = jade.ping().await.unwrap();
    assert_eq!(result, 0);
}

#[cfg(feature = "asyncr")]
#[tokio::test]
async fn async_sign() {
    use lwk_jade::protocol::DebugSetMnemonicParams;

    lwk_test_util::init_logging();

    let docker = clients::Cli::default();

    let container = docker.run(lwk_containers::JadeEmulator);
    let port = container.get_host_port_ipv4(lwk_containers::EMULATOR_PORT);
    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let network = lwk_common::Network::LocaltestLiquid;
    let jade = lwk_jade::asyncr::Jade::new_tcp(stream, network);
    let params = DebugSetMnemonicParams {
        mnemonic: TEST_MNEMONIC.to_string(),
        passphrase: None,
        temporary_wallet: false,
    };
    jade.debug_set_mnemonic(params).await.unwrap();

    let result = jade.ping().await.unwrap();
    assert_eq!(result, 0);

    let pset_base64 = include_str!("../test_data/pset_to_be_signed.base64");
    let mut pset: PartiallySignedTransaction = pset_base64.parse().unwrap();
    assert_eq!(pset.outputs().len(), 3);

    let sign = jade.sign(&mut pset).await.unwrap();
    assert!(sign > 0);
}

fn mock_version_info() -> VersionInfoResult {
    VersionInfoResult {
        jade_version: "1".to_string(),
        jade_ota_max_chunk: 4096,
        jade_config: "NORADIO".to_string(),
        board_type: "DEV".to_string(),
        jade_features: "DEV".to_string(),
        idf_version: "v5.1.2".to_string(),
        chip_features: "32000000".to_string(),
        efusemac: "000000000000".to_string(),
        battery_status: 0,
        jade_state: JadeState::Ready,
        jade_networks: "ALL".to_string(),
        jade_has_pin: false,
    }
}
