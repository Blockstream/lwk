use std::net::SocketAddr;
use wollet::ElementsNetwork;

use crate::consts;

#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
    pub datadir: String,
    pub electrum_url: String,
    pub network: ElementsNetwork,
    pub tls: bool,
    pub validate_domain: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir: "/tmp/.ks".into(),
            electrum_url: "".into(),
            network: ElementsNetwork::LiquidTestnet,
            tls: false,
            validate_domain: false,
        }
    }
}

impl Config {
    pub fn default_testnet() -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir: "/tmp/.ks".into(),
            electrum_url: "blockstream.info:465".into(),
            network: ElementsNetwork::LiquidTestnet,
            tls: true,
            validate_domain: true,
        }
    }
}
