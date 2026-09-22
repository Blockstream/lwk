use std::{env, io::Write};

use elements::{
    bitcoin::{NetworkKind, PrivateKey, PublicKey},
    secp256k1_zkp::Secp256k1,
};
use rand::{thread_rng, RngCore};
use tempfile::TempDir;
use testcontainers::core::{ContainerPort, Image, Mount, WaitFor};

pub const PIN_SERVER_PORT: u16 = 8_096;

#[derive(Debug)]
pub struct PinServer {
    name: String,
    tag: String,
    mounts: Vec<Mount>,
    ports: Vec<ContainerPort>,
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

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The container runs uWSGI as www-data, so the bind-mounted key must be
            // world-readable; the containing temporary directory limits host access.
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&file_path, perms)?;
        }

        let prv_key = PrivateKey::from_slice(&random_buff, NetworkKind::Test).expect("32 bytes");
        let pin_server_pub_key = PublicKey::from_private_key(&Secp256k1::new(), &prv_key);

        assert!(file_path.is_absolute() && file_path.exists());

        let mounts = vec![
            Mount::bind_mount(
                file_path.display().to_string(),
                format!("/{SERVER_PRIVATE_KEY}"),
            ),
            Mount::bind_mount(dir.path().display().to_string(), format!("/{PINS}")),
        ];

        Ok(Self {
            name: env::var("PIN_SERVER_IMAGE_NAME").unwrap_or("tulipan81/blind_pin_server".into()),
            tag: env::var("PIN_SERVER_IMAGE_VERSION").unwrap_or("v0.0.7".into()),
            mounts,
            ports: vec![ContainerPort::from(PIN_SERVER_PORT)],
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
    fn name(&self) -> &str {
        &self.name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("run: wsgi:")]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &self.ports
    }

    fn mounts(&self) -> impl IntoIterator<Item = &Mount> {
        &self.mounts
    }
}
