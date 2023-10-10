use std::net::SocketAddr;

use crate::consts;

#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
        }
    }
}
