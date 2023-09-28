use std::{collections::HashMap, io::Write};

use bitcoin::{
    hashes::{hex::FromHex, sha256, Hash},
    secp256k1::{ecdsa::Signature, Message, Secp256k1},
    PrivateKey, PublicKey,
};
use jade::protocol::HandshakeParams;
use rand::{thread_rng, RngCore};
use tempfile::{tempdir, TempDir};
use testcontainers::{clients, core::WaitFor, Image, ImageArgs};

pub const PORT: u16 = 8_096;

#[derive(Debug)]
pub struct PinServerEmulator {
    volumes: HashMap<String, String>,
    _dir: TempDir,
    pub_key: PublicKey,
}

impl PinServerEmulator {
    pub fn pub_key(&self) -> &PublicKey {
        &self.pub_key
    }
}

const SERVER_PRIVATE_KEY: &str = "server_private_key.key";
const PINS: &str = "pins";

impl Default for PinServerEmulator {
    fn default() -> Self {
        // docker run -v $PWD/server_private_key.key:/server_private_key.key -v $PWD/pinsdir:/pins -p 8096:8096 xenoky/dockerized_pinserver

        let dir = tempdir().unwrap();
        let file_path = dir.path().join(SERVER_PRIVATE_KEY);
        let mut file = std::fs::File::create(&file_path).unwrap();
        let mut random_buff = [0u8; 32];
        let mut rng = thread_rng();
        rng.fill_bytes(&mut random_buff);
        file.write_all(&random_buff).unwrap();

        let prv_key = PrivateKey::from_slice(&random_buff, bitcoin::Network::Regtest).unwrap();
        let pub_key = PublicKey::from_private_key(&Secp256k1::new(), &prv_key);

        let mut volumes = HashMap::new();
        volumes.insert(
            format!("{}", file_path.display()),
            format!("/{}", SERVER_PRIVATE_KEY),
        );
        volumes.insert(format!("{}", dir.path().display()), format!("/{}", PINS));

        Self {
            volumes,
            _dir: dir,
            pub_key,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Args;

impl ImageArgs for Args {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        let args = ["bash".to_string()];
        Box::new(args.into_iter())
    }
}

impl Image for PinServerEmulator {
    type Args = ();

    fn name(&self) -> String {
        "tulipan81/blind_pin_server".into()
    }

    fn tag(&self) -> String {
        "v0.0.3".into()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdOutMessage {
            message: "run: wsgi:".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [PORT].into()
    }

    fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.volumes.iter())
    }
}

#[test]
fn pin_server() {
    let docker = clients::Cli::default();
    let pin_server = PinServerEmulator::default();
    let pin_server_pub_key = *pin_server.pub_key();
    let container = docker.run(pin_server);

    let port = container.get_host_port_ipv4(PORT);
    let pin_server_url = format!("http://127.0.0.1:{port}");
    let result = ureq::get(&pin_server_url).call().unwrap();
    assert_eq!(result.status(), 200);

    let start_handshake_url = format!("{pin_server_url}/start_handshake");
    let resp = ureq::post(&start_handshake_url).call().unwrap();
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
