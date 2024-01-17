use jade::Network as JadeNetwork;
use jade::TIMEOUT;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use wollet::elements::AssetId;
use wollet::ElementsNetwork;

use crate::{consts, Error};

#[derive(Clone, Debug)]
pub struct Config {
    /// The address where the RPC server is listening or the client is connecting to
    pub addr: SocketAddr,
    pub datadir: PathBuf,
    pub electrum_url: String,
    pub network: ElementsNetwork,
    pub tls: bool,
    pub validate_domain: bool,
    pub explorer_url: String,
    pub timeout: Duration,
}

impl Config {
    pub fn default_testnet(datadir: PathBuf) -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            electrum_url: "blockstream.info:465".into(),
            network: ElementsNetwork::LiquidTestnet,
            tls: true,
            validate_domain: true,
            explorer_url: "https://blockstream.info/liquidtestnet/".into(),
            timeout: TIMEOUT,
        }
    }

    pub fn default_mainnet(datadir: PathBuf) -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            electrum_url: "blockstream.info:995".into(),
            network: ElementsNetwork::Liquid,
            tls: true,
            validate_domain: true,
            explorer_url: "https://blockstream.info/liquid/".into(),
            timeout: TIMEOUT,
        }
    }

    pub fn default_regtest(electrum_url: &str, datadir: PathBuf) -> Self {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset = AssetId::from_str(policy_asset).expect("static");
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            electrum_url: electrum_url.into(),
            network: ElementsNetwork::ElementsRegtest { policy_asset },
            tls: false,
            validate_domain: false,
            explorer_url: "".into(),
            timeout: TIMEOUT,
        }
    }

    pub fn jade_network(&self) -> JadeNetwork {
        match self.network {
            ElementsNetwork::Liquid => JadeNetwork::Liquid,
            ElementsNetwork::LiquidTestnet => JadeNetwork::TestnetLiquid,
            ElementsNetwork::ElementsRegtest { .. } => JadeNetwork::LocaltestLiquid,
        }
    }

    pub fn default_home() -> Result<PathBuf, Error> {
        let mut path = home::home_dir().ok_or(Error::Generic("Cannot get home dir".into()))?;
        path.push(".ks");
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// Appends the network to the given datadir
    pub fn datadir(&self) -> Result<PathBuf, Error> {
        let mut path: PathBuf = self.datadir.clone();
        path.push(self.network.as_str());
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// Returns the path of the state file under datadir
    pub fn state_path(&self) -> Result<PathBuf, Error> {
        let mut path = self.datadir()?;
        path.push("state.json");
        Ok(path)
    }

    /// True if Liquid mainnet
    pub fn is_mainnet(&self) -> bool {
        matches!(self.network, ElementsNetwork::Liquid)
    }
}
