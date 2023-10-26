use std::net::SocketAddr;

use crate::consts;

#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
    pub datadir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir: "/tmp/.ks".into(),
        }
    }
}
