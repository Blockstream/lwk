use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use connection::Connection;
use protocol::{
    AuthResult, AuthUserParams, BoolResult, EntropyParams, EpochParams, HandshakeData,
    HandshakeParams, Network, Params, PingResult, Request, Response, VersionInfoResult,
};
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;

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

    fn send_request<T>(&mut self, method: &str, params: Option<Params>) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        let mut rng = rand::thread_rng();
        let id = rng.next_u32().to_string();
        dbg!(&id);
        let req = Request {
            id,
            method: method.into(),
            params,
        };
        let mut buf = Vec::new();
        ciborium::into_writer(&req, &mut buf).unwrap();
        dbg!(buf.len());
        dbg!(hex::encode(&buf));

        self.conn.write_all(&buf).unwrap();
        thread::sleep(Duration::from_millis(1000));

        let mut rx = vec![0; 1000];

        let mut total = 0;
        loop {
            match self.conn.read(&mut rx[total..]) {
                Ok(len) => {
                    dbg!(&len);
                    total += len;
                    dbg!(&total);
                    dbg!(hex::encode(&rx[..total]));
                    match ciborium::from_reader::<Response<T>, &[u8]>(&rx[..total]) {
                        Ok(r) => return Ok(r.result.unwrap()),
                        Err(e) => {
                            dbg!(e);
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
