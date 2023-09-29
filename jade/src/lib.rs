use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use connection::Connection;
use protocol::{
    AuthResult, AuthUserParams, BoolResult, EntropyParams, EpochParams, HandshakeData,
    HandshakeParams, Network, Params, PingResult, Request, Response, UpdatePinserverParams,
    VersionInfoResult,
};
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;

use crate::error::Error;

pub mod connection;
pub mod error;
pub mod protocol;

pub type Result<T> = std::result::Result<T, error::Error>;

pub struct Jade {
    conn: Connection,
}

impl Jade {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn ping(&mut self) -> Result<PingResult> {
        self.send_request("ping", None)
    }

    pub fn logout(&mut self) -> Result<BoolResult> {
        self.send_request("logout", None)
    }

    pub fn version_info(&mut self) -> Result<VersionInfoResult> {
        self.send_request("get_version_info", None)
    }

    pub fn set_epoch(&mut self, epoch: u64) -> Result<BoolResult> {
        let params = Params::Epoch(EpochParams { epoch });
        self.send_request("set_epoch", Some(params))
    }

    pub fn add_entropy(&mut self, entropy: &[u8]) -> Result<BoolResult> {
        let params = Params::Entropy(EntropyParams {
            entropy: ByteBuf::from(entropy),
        });
        self.send_request("add_entropy", Some(params))
    }

    pub fn auth_user(&mut self, network: Network) -> Result<AuthResult<String>> {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error::Error::SystemTimeError)?
            .as_secs();
        let params = Params::AuthUser(AuthUserParams {
            network: network.into(),
            epoch,
        });
        self.send_request("auth_user", Some(params))
    }

    pub fn handshake_init(&mut self, params: HandshakeParams) -> Result<AuthResult<HandshakeData>> {
        let params = Params::Handshake(params);
        self.send_request("handshake_init", Some(params))
    }

    pub fn update_pinserver(&mut self, params: UpdatePinserverParams) -> Result<BoolResult> {
        let params = Params::UpdatePinServer(params);
        self.send_request("update_pinserver", Some(params))
    }

    fn send_request<T>(&mut self, method: &str, params: Option<Params>) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        let mut rng = rand::thread_rng();
        let id = rng.next_u32().to_string();
        let req = Request {
            id,
            method: method.into(),
            params,
        };
        let mut buf = Vec::new();
        ciborium::into_writer(&req, &mut buf).unwrap();
        println!(
            "\n--->\t{:?}\n\t({} bytes) {}",
            &req,
            buf.len(),
            hex::encode(&buf)
        );

        self.conn.write_all(&buf).unwrap();
        thread::sleep(Duration::from_millis(1000));

        let mut rx = vec![0; 1000];

        let mut total = 0;
        loop {
            match self.conn.read(&mut rx[total..]) {
                Ok(len) => {
                    total += len;
                    match ciborium::from_reader::<Response<T>, &[u8]>(&rx[..total]) {
                        Ok(r) => {
                            if let Some(result) = r.result {
                                println!(
                                    "\n<---\t{:?}\n\t({} bytes) {}",
                                    &result,
                                    total,
                                    hex::encode(&rx[..total])
                                );
                                return Ok(result);
                            }
                            if let Some(error) = r.error {
                                return Err(Error::JadeError(error));
                            }
                            return Err(Error::JadeNeitherErrorNorResult);
                        }
                        Err(e) => {
                            dbg!(e);
                            let generic =
                                ciborium::from_reader::<ciborium::Value, &[u8]>(&rx[..total]);
                            dbg!(&generic);
                            generic.unwrap();
                        }
                    }
                }
                Err(e) => {
                    dbg!(e);
                    todo!();
                }
            }
        }
    }
}
