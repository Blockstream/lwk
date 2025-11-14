use std::{collections::HashMap, env, io::Write};

use elements::{
    bitcoin::{NetworkKind, PrivateKey, PublicKey},
    secp256k1_zkp::Secp256k1,
};
use rand::{thread_rng, RngCore};
use tempfile::TempDir;
use testcontainers::{core::WaitFor, Image};

pub const PIN_SERVER_PORT: u16 = 8_096;

#[derive(Debug)]
pub struct PinServer {
    volumes: HashMap<String, String>,
    pub_key: PublicKey,
}

impl PinServer {
    pub fn pub_key(&self) -> &PublicKey {
        &self.pub_key
    }
}

const SERVER_PRIVATE_KEY: &str = "server_private_key.key";
const PINS: &str = "pins";

impl PinServer {
    /// Create a PinServerEmulator
    ///
    /// takes the temporary directory as parameter to ensure it's not deleted when the
    /// docker containers runtime convert the Image into RunnableImage, consuming the struct
    pub fn new(dir: &TempDir) -> Result<Self, std::io::Error> {
        // docker run -v $PWD/server_private_key.key:/server_private_key.key -v $PWD/pinsdir:/pins -p 8096:8096 xenoky/dockerized_pinserver

        let file_path = dir.path().join(SERVER_PRIVATE_KEY);
        let mut file = std::fs::File::create(&file_path)?;
        let mut random_buff = [0u8; 32];
        let mut rng = thread_rng();
        rng.fill_bytes(&mut random_buff);
        file.write_all(&random_buff)?;

        let prv_key = PrivateKey::from_slice(&random_buff, NetworkKind::Test).expect("32 bytes");
        let pin_server_pub_key = PublicKey::from_private_key(&Secp256k1::new(), &prv_key);

        assert!(file_path.is_absolute() && file_path.exists());

        let mut volumes = HashMap::new();
        volumes.insert(
            format!("{}", file_path.display()),
            format!("/{SERVER_PRIVATE_KEY}"),
        );
        volumes.insert(format!("{}", dir.path().display()), format!("/{PINS}"));

        Ok(Self {
            volumes,
            pub_key: pin_server_pub_key,
        })
    }

    /// Creates a tempdir
    ///
    /// under gitlab env using tempdir may cause issue, thus in this env the temp dir is created
    /// under the project dir
    pub fn tempdir() -> Result<TempDir, std::io::Error> {
        match std::env::var("CI_PROJECT_DIR") {
            Ok(var) => TempDir::new_in(var),
            Err(_) => tempfile::tempdir(),
        }
    }
}

impl Image for PinServer {
    type Args = ();

    fn name(&self) -> String {
        env::var("PIN_SERVER_IMAGE_NAME").unwrap_or("tulipan81/blind_pin_server".into())
    }

    fn tag(&self) -> String {
        env::var("PIN_SERVER_IMAGE_VERSION").unwrap_or("v0.0.7".into())
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
