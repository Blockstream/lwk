use std::{collections::HashMap, env, io::Write};

use bitcoin::{secp256k1::Secp256k1, PrivateKey, PublicKey};
use rand::{thread_rng, RngCore};
use tempfile::TempDir;
use testcontainers::{core::WaitFor, Image, ImageArgs};

pub const PIN_SERVER_PORT: u16 = 8_096;

#[derive(Debug)]
pub struct PinServerEmulator {
    volumes: HashMap<String, String>,
    pub_key: PublicKey,
}

impl PinServerEmulator {
    pub fn pub_key(&self) -> &PublicKey {
        &self.pub_key
    }
}

const SERVER_PRIVATE_KEY: &str = "server_private_key.key";
const PINS: &str = "pins";

impl PinServerEmulator {
    pub fn new(dir: &TempDir) -> Self {
        // docker run -v $PWD/server_private_key.key:/server_private_key.key -v $PWD/pinsdir:/pins -p 8096:8096 xenoky/dockerized_pinserver

        let file_path = dir.path().join(SERVER_PRIVATE_KEY);
        let mut file = std::fs::File::create(&file_path).unwrap();
        let mut random_buff = [0u8; 32];
        let mut rng = thread_rng();
        rng.fill_bytes(&mut random_buff);
        file.write_all(&random_buff).unwrap();

        let prv_key = PrivateKey::from_slice(&random_buff, bitcoin::Network::Regtest).unwrap();
        let pub_key = PublicKey::from_private_key(&Secp256k1::new(), &prv_key);

        assert!(file_path.is_absolute() && file_path.exists());

        let mut volumes = HashMap::new();
        volumes.insert(
            format!("{}", file_path.display()),
            format!("/{}", SERVER_PRIVATE_KEY),
        );
        volumes.insert(format!("{}", dir.path().display()), format!("/{}", PINS));

        Self { volumes, pub_key }
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
        env::var("PIN_SERVER_IMAGE_NAME").unwrap_or("tulipan81/blind_pin_server".into())
    }

    fn tag(&self) -> String {
        env::var("PIN_SERVER_IMAGE_NAME").unwrap_or("v0.0.3".into())
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdOutMessage {
            message: "run: wsgi:".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [PIN_SERVER_PORT].into()
    }

    fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.volumes.iter())
    }
}
