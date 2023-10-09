use bs_containers::testcontainers::clients::Cli;
use bs_containers::{
    pin_server::{PinServerEmulator, PIN_SERVER_PORT},
    print_docker_logs_and_panic,
};
use elements::bitcoin::{
    hashes::{hex::FromHex, sha256, Hash},
    secp256k1::{ecdsa::Signature, Message, Secp256k1},
    PublicKey,
};
use jade::protocol::HandshakeParams;

#[test]
fn pin_server() {
    let docker = Cli::default();
    let tempdir = PinServerEmulator::tempdir();
    let pin_server = PinServerEmulator::new(&tempdir);
    let pin_server_pub_key = *pin_server.pub_key();
    let container = docker.run(pin_server);

    let port = container.get_host_port_ipv4(PIN_SERVER_PORT);
    let pin_server_url = format!("http://127.0.0.1:{port}");
    let result = ureq::get(&pin_server_url).call().unwrap();
    assert_eq!(result.status(), 200);

    let start_handshake_url = format!("{pin_server_url}/start_handshake");
    let resp = ureq::post(&start_handshake_url)
        .call()
        .unwrap_or_else(|_| print_docker_logs_and_panic(container.id()));
    let params: HandshakeParams = resp.into_json().unwrap();
    verify(&params, &pin_server_pub_key);
}

pub fn verify(params: &HandshakeParams, pin_server_pub_key: &PublicKey) {
    let ske_bytes = Vec::<u8>::from_hex(&params.ske).unwrap();
    let ske_hash = sha256::Hash::hash(&ske_bytes);

    let signature_bytes = Vec::<u8>::from_hex(&params.sig).unwrap();
    let signature = Signature::from_compact(&signature_bytes).unwrap();

    let message = Message::from_slice(&ske_hash[..]).unwrap();

    let verify = Secp256k1::verification_only()
        .verify_ecdsa(&message, &signature, &pin_server_pub_key.inner)
        .is_ok();
    assert!(verify);
}