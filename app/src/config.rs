use jade::Network as JadeNetwork;
use std::net::SocketAddr;
use std::str::FromStr;
use wollet::elements::AssetId;
use wollet::ElementsNetwork;

use crate::consts;

#[derive(Clone, Debug)]
pub struct Config {
    /// The address where the RPC server is listening or the client is connecting to
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

    pub fn default_mainnet() -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir: "/tmp/.ks".into(),
            electrum_url: "blockstream.info:995".into(),
            network: ElementsNetwork::Liquid,
            tls: true,
            validate_domain: true,
        }
    }

    pub fn default_regtest(electrum_url: &str) -> Self {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset = AssetId::from_str(policy_asset).unwrap();
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir: "/tmp/.ks".into(),
            electrum_url: electrum_url.into(),
            network: ElementsNetwork::ElementsRegtest { policy_asset },
            tls: false,
            validate_domain: false,
        }
    }

    pub fn jade_network(&self) -> JadeNetwork {
        match self.network {
            ElementsNetwork::Liquid => JadeNetwork::Liquid,
            ElementsNetwork::LiquidTestnet => JadeNetwork::TestnetLiquid,
            ElementsNetwork::ElementsRegtest { .. } => JadeNetwork::LocaltestLiquid,
        }
    }
}
