use bs_containers::testcontainers::clients::Cli;
use bs_containers::{PinServer, PIN_SERVER_PORT};
use elements::bitcoin::{
    hashes::{hex::FromHex, sha256, Hash},
    secp256k1::{ecdsa::Signature, Message, Secp256k1},
    PublicKey,
};
use jade::protocol::HandshakeInitParams;

#[test]
fn pin_server() {
    let docker = Cli::default();
    let tempdir = PinServer::tempdir();
    let pin_server = PinServer::new(&tempdir);
    let pin_server_pub_key = *pin_server.pub_key();
    let container = docker.run(pin_server);

    let port = container.get_host_port_ipv4(PIN_SERVER_PORT);
    let pin_server_url = format!("http://127.0.0.1:{port}");
    let result = minreq::get(&pin_server_url).send().unwrap();
    assert_eq!(result.status_code, 200);

    let start_handshake_url = format!("{pin_server_url}/start_handshake");
    let resp = minreq::post(start_handshake_url).send().unwrap();
    let params: HandshakeInitParams = serde_json::from_slice(resp.as_bytes()).unwrap();
    verify(&params, &pin_server_pub_key);
}

pub fn verify(params: &HandshakeInitParams, pin_server_pub_key: &PublicKey) {
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
