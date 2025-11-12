use lwk_common::electrum_ssl::LIQUID_SOCKET;
use lwk_common::electrum_ssl::LIQUID_TESTNET_SOCKET;
use lwk_common::Network as JadeNetwork;
use lwk_jade::TIMEOUT;
use lwk_wollet::elements::AssetId;
use lwk_wollet::ElementsNetwork;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use crate::{consts, Error};

#[derive(Clone, Debug)]
pub struct Config {
    /// The address where the RPC server is listening or the client is connecting to
    pub addr: SocketAddr,
    pub datadir: PathBuf,
    pub server_url: String,
    pub network: ElementsNetwork,

    pub explorer_url: String,

    pub registry_url: String,
    pub timeout: Duration,
    pub scanning_interval: Duration,
}

impl Config {
    pub fn default_testnet(datadir: PathBuf) -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            server_url: format!("ssl://{LIQUID_TESTNET_SOCKET}"),
            network: ElementsNetwork::LiquidTestnet,
            explorer_url: "https://blockstream.info/liquidtestnet/".into(),
            registry_url: "https://assets-testnet.blockstream.info/".into(),
            timeout: TIMEOUT,
            scanning_interval: consts::SCANNING_INTERVAL,
        }
    }

    pub fn default_mainnet(datadir: PathBuf) -> Self {
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            server_url: format!("ssl://{LIQUID_SOCKET}"),
            network: ElementsNetwork::Liquid,
            explorer_url: "https://blockstream.info/liquid/".into(),
            registry_url: "https://assets.blockstream.info/".into(),
            timeout: TIMEOUT,
            scanning_interval: consts::SCANNING_INTERVAL,
        }
    }

    /// For regtest there are no reasonable default for `electrum_url`, `explorer_url`, and `registry_url`
    /// It will be caller responsability to mutate them according to regtest env
    pub fn default_regtest(datadir: PathBuf) -> Self {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset = AssetId::from_str(policy_asset).expect("static");
        Self {
            addr: consts::DEFAULT_ADDR.into(),
            datadir,
            server_url: "".into(),
            network: ElementsNetwork::ElementsRegtest { policy_asset },
            explorer_url: "".into(),
            registry_url: "".into(),
            timeout: TIMEOUT,
            // Scan more frequently while testing
            scanning_interval: Duration::from_secs(1),
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
        path.push(".lwk");
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

    pub fn blockchain_client(
        &self,
    ) -> Result<impl lwk_wollet::clients::blocking::BlockchainBackend, Error> {
        // TODO cache it instead of recreating every time
        let electrum_url = self.server_url.parse().map_err(lwk_wollet::Error::Url)?;
        Ok(lwk_wollet::ElectrumClient::new(&electrum_url)?)
    }
}
