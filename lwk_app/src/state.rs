use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lwk_common::Signer;
use lwk_jade::Jade;
use lwk_rpc_model::request;
use lwk_signer::AnySigner;
use lwk_signer::SwSigner;
use lwk_tiny_jrpc::Request;
use lwk_wollet::bitcoin::bip32::{Fingerprint, Xpub};
use lwk_wollet::bitcoin::XKeyIdentifier;
use lwk_wollet::elements::encode::serialize;
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::pset::elip100::AssetMetadata;
use lwk_wollet::elements::{Address, AssetId, Transaction, Txid};
use lwk_wollet::Contract;
use lwk_wollet::RegistryAssetData;
use lwk_wollet::Wollet;
use serde::Serialize;

use crate::config::Config;
use crate::method::Method;
use crate::Error;

#[derive(Debug)]
enum AppSignerInner {
    #[allow(dead_code)]
    // allow dead_code otherwise warning on the second parameter which is needed only with the serial feature
    JadeId(XKeyIdentifier, lwk_common::Network),

    AvailableSigner(AnySigner),
    ExternalSigner(Fingerprint),
}

#[derive(Debug)]
pub struct AppSigner {
    inner: AppSignerInner,
    persist: bool,
}

impl AppSigner {
    pub fn new_sw(mnemonic: &str, is_mainnet: bool, persist: bool) -> Result<Self, Error> {
        let sw = SwSigner::new(mnemonic, is_mainnet)?;
        let inner = AppSignerInner::AvailableSigner(AnySigner::Software(sw));
        Ok(AppSigner { inner, persist })
    }

    pub fn new_jade(
        id: XKeyIdentifier,
        emulator: Option<SocketAddr>,
        network: lwk_common::Network,
    ) -> Result<Self, Error> {
        let inner = if let Some(socket) = emulator {
            // The emulator is meant to be used only in testing, we don't aim to handle connection/disconnection
            let jade = Jade::from_socket(socket, network)?;
            AppSignerInner::AvailableSigner(AnySigner::Jade(jade, id))
        } else {
            AppSignerInner::JadeId(id, network)
        };
        Ok(AppSigner {
            inner,
            persist: true,
        })
    }

    pub fn new_external(fingerprint: Fingerprint) -> Self {
        AppSigner {
            inner: AppSignerInner::ExternalSigner(fingerprint),
            persist: false,
        }
    }

    pub fn fingerprint(&self) -> Result<Fingerprint, Error> {
        Ok(match &self.inner {
            AppSignerInner::AvailableSigner(s) => s.fingerprint()?,
            AppSignerInner::ExternalSigner(f) => *f,
            AppSignerInner::JadeId(id, _) => id_to_fingerprint(id),
        })
    }

    pub fn xpub(&self) -> Result<Option<Xpub>, Error> {
        Ok(match &self.inner {
            AppSignerInner::AvailableSigner(s) => Some(s.xpub()?),
            _ => None,
        })
    }

    pub fn id(&self) -> Result<Option<XKeyIdentifier>, Error> {
        Ok(match &self.inner {
            AppSignerInner::AvailableSigner(s) => Some(s.identifier()?),
            AppSignerInner::JadeId(id, _) => Some(*id),
            _ => None,
        })
    }

    pub fn mnemonic(&self) -> Option<String> {
        match &self.inner {
            AppSignerInner::AvailableSigner(AnySigner::Software(s)) => {
                s.mnemonic().map(|m| m.to_string())
            }
            _ => None,
        }
    }

    pub fn type_(&self) -> String {
        match &self.inner {
            AppSignerInner::ExternalSigner(_) => "external".into(),
            AppSignerInner::JadeId(_, _) => "jade-id".into(),
            AppSignerInner::AvailableSigner(AnySigner::Software(_)) => "software".into(),
            AppSignerInner::AvailableSigner(AnySigner::Jade(_, _)) => "jade".into(),
            #[allow(unreachable_patterns)]
            _ => todo!(),
        }
    }
}

// TODO upstream as method of XKeyIdentifier to rust-bitcoin
pub fn id_to_fingerprint(id: &XKeyIdentifier) -> Fingerprint {
    id[0..4].try_into().expect("4 is the fingerprint length")
}

pub enum AppAsset {
    #[allow(dead_code)]
    /// The policy asset (L-BTC)
    PolicyAsset(AssetId),

    /// An asset with contract committed to it
    RegistryAsset(RegistryAssetData),

    /// A reissuance token for an asset
    ReissuanceToken(RegistryAssetData),
}

impl AppAsset {
    pub fn name(&self) -> String {
        match self {
            AppAsset::PolicyAsset(_) => "liquid bitcoin".into(),
            AppAsset::RegistryAsset(d) => d.contract().name.clone(),
            AppAsset::ReissuanceToken(d) => {
                format!("reissuance token for {}", d.contract().name)
            }
        }
    }

    pub fn ticker(&self) -> String {
        match self {
            AppAsset::PolicyAsset(_) => "L-BTC".into(),
            AppAsset::RegistryAsset(d) => d.contract().ticker.clone(),
            AppAsset::ReissuanceToken(d) => {
                format!("reissuance token for {}", d.contract().ticker)
            }
        }
    }

    pub fn _asset_metadata(&self) -> Option<AssetMetadata> {
        match self {
            AppAsset::PolicyAsset(_) => None,
            AppAsset::RegistryAsset(d) => {
                Some(AssetMetadata::new(d.contract_str(), d.issuance_prevout()))
            }
            AppAsset::ReissuanceToken(d) => {
                Some(AssetMetadata::new(d.contract_str(), d.issuance_prevout()))
            }
        }
    }

    pub fn _asset_id(&self) -> AssetId {
        match self {
            AppAsset::PolicyAsset(asset) => *asset,
            AppAsset::RegistryAsset(d) => d.asset_id(),
            AppAsset::ReissuanceToken(d) => d.token_id(),
        }
    }

    pub fn issuance_tx(&self) -> Option<Transaction> {
        match self {
            AppAsset::RegistryAsset(d) => Some(d.issuance_tx().clone()),
            _ => None,
        }
    }

    pub fn request(&self) -> Option<Request> {
        match self {
            AppAsset::RegistryAsset(a) => {
                let params = request::AssetInsert {
                    asset_id: a.asset_id().to_string(),
                    contract: a.contract_str(),
                    issuance_tx: serialize(a.issuance_tx()).to_hex(),
                };
                Some(Request {
                    jsonrpc: "2.0".into(),
                    id: None,
                    method: Method::AssetInsert.to_string(),
                    params: Some(serde_json::to_value(params).expect("derived")),
                })
            }
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Wollets(HashMap<String, Wollet>);

#[derive(Default)]
pub struct Signers(HashMap<String, AppSigner>);

#[derive(Default)]
pub struct Assets(HashMap<AssetId, AppAsset>);

#[derive(Default)]
pub struct TxMemos(HashMap<String, HashMap<Txid, String>>);

#[derive(Default)]
pub struct AddrMemos(HashMap<String, HashMap<Address, String>>);

pub struct State {
    // TODO: config is read-only, so it's not useful to wrap it in a mutex.
    // Ideally it should be in _another_ struct accessible by method_handler.
    pub config: Config,
    pub wollets: Wollets,
    pub signers: Signers,
    pub assets: Assets,
    pub tx_memos: TxMemos,
    pub addr_memos: AddrMemos,
    pub do_persist: bool,

    /// Number of scan loops started
    pub scan_loops_started: u32,

    /// Number of scan loops completed
    pub scan_loops_completed: u32,

    /// Signal the scanning thread that we don't want to wait anymore
    pub interrupt_wait: bool,
}

impl Wollets {
    pub fn get(&self, name: &str) -> Result<&Wollet, Error> {
        self.0
            .get(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }

    pub fn get_mut(&mut self, name: &str) -> Result<&mut Wollet, Error> {
        self.0
            .get_mut(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }
    pub fn insert(&mut self, name: &str, wollet: Wollet) -> Result<(), Error> {
        if self.0.contains_key(name) {
            return Err(Error::WalletAlreadyLoaded(name.to_string()));
        }

        let first_addr = |w: &Wollet| w.address(Some(0)).map(|a| a.address().clone());
        let other = first_addr(&wollet)?;

        let ours: Vec<_> = self.0.values().map(first_addr).collect::<Result<_, _>>()?;

        let vec: Vec<_> = self
            .0
            .keys()
            .zip(ours.iter())
            .filter(|(_, b)| &other == *b)
            .map(|(n, _)| n)
            .collect();
        if let Some(existing) = vec.first() {
            // TODO: maybe a different error more clear?
            return Err(Error::WalletAlreadyLoaded(existing.to_string()));
        }

        self.0.insert(name.to_string(), wollet);
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<Wollet, Error> {
        self.0
            .remove(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Wollet)> {
        self.0.iter()
    }
}

impl Signers {
    pub fn get(&self, name: &str) -> Result<&AppSigner, Error> {
        self.0
            .get(name)
            .ok_or_else(|| Error::SignerNotExist(name.to_string()))
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, name: &str) -> Result<&mut AppSigner, Error> {
        self.0
            .get_mut(name)
            .ok_or_else(|| Error::SignerNotExist(name.to_string()))
    }

    /// Get an available signer identified by name.
    ///
    /// In some cases, like with a jade not currently linked, it may try to connect to it first
    fn get_available(
        &mut self,
        name: &str,
        timeout: Option<Duration>,
    ) -> Result<&AnySigner, Error> {
        let app_signer = self.get(name)?;
        log::debug!("get_available({name}) return {app_signer:?}");
        let jade = match &app_signer.inner {
            #[cfg(not(feature = "serial"))]
            AppSignerInner::JadeId(_, _) => {
                let _timeout = timeout;
                return Err(Error::FeatSerialDisabled);
            }
            #[cfg(feature = "serial")]
            AppSignerInner::JadeId(id, network) => {
                // try to connect JadeId -> AvailableSigner(Jade)
                // TODO possible errors should be kept
                lwk_jade::Jade::from_serial_matching_id(*network, id, timeout)
                    .map(|jade| AppSignerInner::AvailableSigner(AnySigner::Jade(jade, *id)))
            }
            AppSignerInner::AvailableSigner(AnySigner::Jade(j, id)) => {
                // verify connection, if fails AvailableSigner(Jade) -> JadeId
                if j.unlock().is_err() {
                    // TODO if emulator should throw the error instead of becoming JadeId
                    // TODO ensure identifier it's cached
                    Some(AppSignerInner::JadeId(*id, j.network()))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(inner) = jade {
            let signer = AppSigner {
                inner,
                persist: true,
            };
            // replace the existing AppSignerInner::JadeId with AppSignerInner::AvailableSigner
            self.0.insert(name.to_string(), signer);
        }

        match &self.get(name)?.inner {
            AppSignerInner::AvailableSigner(signer) => Ok(signer),
            AppSignerInner::ExternalSigner(_) => Err(Error::Generic(
                "Invalid operation for external signer".to_string(),
            )),
            AppSignerInner::JadeId(_, _) => Err(Error::Generic(
                "Invalid operation jade is not connected".to_string(),
            )),
        }
    }

    pub fn insert(&mut self, name: &str, signer: AppSigner) -> Result<(), Error> {
        if self.0.contains_key(name) {
            return Err(Error::SignerAlreadyLoaded(name.to_string()));
        }
        let inserting_fingerprint = signer.fingerprint()?;

        // TODO: matchin for fingerprint is not ideal, we could have collisions
        let vec: Vec<_> = self.names_matching_fingerprint(&inserting_fingerprint)?;
        if let Some(existing) = vec.first() {
            // TODO: maybe a different error more clear?
            return Err(Error::SignerAlreadyLoaded(existing.to_string()));
        }

        self.0.insert(name.to_string(), signer);
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<AppSigner, Error> {
        self.0
            .remove(name)
            .ok_or_else(|| Error::SignerNotExist(name.to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &AppSigner)> {
        self.0.iter()
    }

    fn names_matching_fingerprint(&self, fingerprint: &Fingerprint) -> Result<Vec<String>, Error> {
        let fingerprints = self
            .iter()
            .map(|s| s.1.fingerprint())
            .collect::<Result<Vec<Fingerprint>, Error>>()?;

        Ok(self
            .iter()
            .map(|(n, _)| n)
            .zip(fingerprints.iter())
            .filter(|(_, b)| *b == fingerprint)
            .map(|(n, _)| n.clone())
            .collect())
    }

    /// Get a name from the fingerprint
    pub fn name_from_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        warnings: &mut Vec<String>,
    ) -> Option<String> {
        let names = self.names_matching_fingerprint(fingerprint).ok()?;

        match names.len() {
            0 => None,
            1 => Some(names[0].clone()),
            _ => {
                warnings.push(format!(
                    "{fingerprint} corresponds to multiple loaded signers"
                ));
                None
            }
        }
    }
}

impl Assets {
    pub fn iter(&self) -> impl Iterator<Item = (&AssetId, &AppAsset)> {
        self.0.iter()
    }
}

impl TxMemos {
    // TODO; return Option<&HashMap<Txid, String>>
    pub fn for_wollet(&self, wollet: &str) -> HashMap<Txid, String> {
        self.0.get(wollet).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, wollet: &str, txid: &Txid, memo: &str) -> Result<(), Error> {
        if let Some(wollet_memos) = self.0.get_mut(wollet) {
            wollet_memos.insert(*txid, memo.to_string());
        } else {
            let mut wollet_memos = HashMap::new();
            wollet_memos.insert(*txid, memo.to_string());
            self.0.insert(wollet.to_string(), wollet_memos);
        }
        Ok(())
    }

    pub fn remove(&mut self, wollet: &str) {
        self.0.remove(wollet);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &HashMap<Txid, String>)> {
        self.0.iter()
    }
}

impl AddrMemos {
    // TODO; return Option<&HashMap<Address, String>>
    pub fn for_wollet(&self, wollet: &str) -> HashMap<Address, String> {
        self.0.get(wollet).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, wollet: &str, addr: &Address, memo: &str) -> Result<(), Error> {
        if let Some(wollet_memos) = self.0.get_mut(wollet) {
            wollet_memos.insert(addr.clone(), memo.to_string());
        } else {
            let mut wollet_memos = HashMap::new();
            wollet_memos.insert(addr.clone(), memo.to_string());
            self.0.insert(wollet.to_string(), wollet_memos);
        }
        Ok(())
    }

    pub fn remove(&mut self, wollet: &str) {
        self.0.remove(wollet);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &HashMap<Address, String>)> {
        self.0.iter()
    }
}

impl State {
    pub fn insert_policy_asset(&mut self) {
        let asset_id = self.config.network.policy_asset();
        self.assets
            .0
            .insert(asset_id, AppAsset::PolicyAsset(asset_id));
    }

    pub fn get_asset(&self, asset: &AssetId) -> Result<&AppAsset, Error> {
        self.assets
            .0
            .get(asset)
            .ok_or_else(|| Error::AssetNotExist(asset.to_string()))
    }

    pub fn insert_asset(
        &mut self,
        asset_id: AssetId,
        issuance_tx: Transaction,
        contract: Contract,
    ) -> Result<(), Error> {
        let data = RegistryAssetData::new(asset_id, issuance_tx, contract)?;
        self.assets
            .0
            .insert(asset_id, AppAsset::RegistryAsset(data.clone()));
        self.assets
            .0
            .insert(data.token_id(), AppAsset::ReissuanceToken(data));
        Ok(())
    }

    pub fn remove_asset(&mut self, asset: &AssetId) -> Result<(), Error> {
        self.assets
            .0
            .remove(asset)
            .ok_or_else(|| Error::AssetNotExist(asset.to_string()))?;
        Ok(())
    }

    fn get_asset_from_str(&self, asset: &str) -> Result<&AppAsset, Error> {
        let asset = AssetId::from_str(asset).map_err(|e| Error::Generic(e.to_string()))?;
        self.get_asset(&asset)
    }

    pub fn get_issuance_tx(&self, asset: &AssetId) -> Option<Transaction> {
        self.get_asset(asset).ok().and_then(|a| a.issuance_tx())
    }

    pub fn replace_id_with_ticker(
        &self,
        balance: impl IntoIterator<Item = (String, i64)>,
    ) -> HashMap<String, i64> {
        balance
            .into_iter()
            .map(|(k, v)| {
                let t = self.get_asset_from_str(&k).map(|a| a.ticker()).unwrap_or(k);
                (t, v)
            })
            .collect()
    }

    pub fn persist<T: Serialize>(&mut self, data: T) -> Result<(), Error> {
        if self.do_persist {
            let data = serde_json::to_string(&data)?;
            let path = self.config.state_path()?;
            let mut file = OpenOptions::new()
                .create_new(!path.exists())
                .append(true)
                .open(path)?;
            writeln!(file, "{data}")?;
            file.sync_all()?;
        }
        Ok(())
    }

    pub fn persist_all(&mut self) -> Result<(), Error> {
        let path = self.config.state_path()?;
        let mut temp = path.clone();
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Clock may have gone backwards")
            .as_millis();
        temp.set_file_name(millis.to_string());
        let mut file = File::create(&temp)?;
        for req in self.as_requests()? {
            let data = serde_json::to_string(&req)?;
            writeln!(file, "{data}")?;
        }
        std::fs::rename(temp, path)?;
        file.sync_all()?;

        Ok(())
    }

    fn as_requests(&self) -> Result<Vec<Request>, Error> {
        let mut requests = vec![];

        // Wollets
        for (n, w) in self.wollets.iter() {
            let params = request::WalletLoad {
                descriptor: w.wollet_descriptor().to_string(),
                name: n.to_string(),
            };
            let r = Request {
                jsonrpc: "2.0".into(),
                id: None,
                method: Method::WalletLoad.to_string(),
                params: Some(serde_json::to_value(params)?),
            };
            requests.push(r);
        }

        // Tx memos
        for (name, wollet_memos) in self.tx_memos.iter() {
            for (txid, memo) in wollet_memos.iter() {
                let params = request::WalletSetTxMemo {
                    name: name.to_string(),
                    txid: txid.to_string(),
                    memo: memo.to_string(),
                };
                let r = Request {
                    jsonrpc: "2.0".into(),
                    id: None,
                    method: Method::WalletSetTxMemo.to_string(),
                    params: Some(serde_json::to_value(params)?),
                };
                requests.push(r);
            }
        }

        // Addr memos
        for (name, wollet_memos) in self.addr_memos.iter() {
            for (address, memo) in wollet_memos.iter() {
                let params = request::WalletSetAddrMemo {
                    name: name.to_string(),
                    address: address.to_string(),
                    memo: memo.to_string(),
                };
                let r = Request {
                    jsonrpc: "2.0".into(),
                    id: None,
                    method: Method::WalletSetAddrMemo.to_string(),
                    params: Some(serde_json::to_value(params)?),
                };
                requests.push(r);
            }
        }

        // Signers
        for (n, s) in self.signers.iter() {
            let (params, method) = match &s.inner {
                AppSignerInner::JadeId(id, _) => {
                    let params = request::SignerLoadJade {
                        name: n.to_string(),
                        id: id.to_string(),
                        emulator: None, // ?
                    };
                    (serde_json::to_value(params)?, Method::SignerLoadJade)
                }
                AppSignerInner::AvailableSigner(a) => match a {
                    AnySigner::Software(a) => {
                        let params = request::SignerLoadSoftware {
                            name: n.to_string(),
                            mnemonic: a
                                .mnemonic()
                                .expect("we only create signers from mnemonic")
                                .to_string(),
                            persist: s.persist,
                        };
                        (serde_json::to_value(params)?, Method::SignerLoadSoftware)
                    }
                    AnySigner::Jade(_, id) => {
                        let params = request::SignerLoadJade {
                            name: n.to_string(),
                            id: id.to_string(),
                            emulator: None, // ?
                        };
                        (serde_json::to_value(params)?, Method::SignerLoadJade)
                    }
                    #[allow(unreachable_patterns)]
                    _ => todo!(),
                },
                AppSignerInner::ExternalSigner(f) => {
                    let params = request::SignerLoadExternal {
                        name: n.to_string(),
                        fingerprint: f.to_string(),
                    };
                    (serde_json::to_value(params)?, Method::SignerLoadExternal)
                }
            };

            let r = Request {
                jsonrpc: "2.0".into(),
                id: None,
                method: method.to_string(),
                params: Some(params),
            };
            requests.push(r);
        }

        // Assets
        for (_, a) in self.assets.iter() {
            if let Some(r) = a.request() {
                requests.push(r);
            }
        }

        Ok(requests)
    }

    pub fn registry_asset_data(&self) -> impl Iterator<Item = &RegistryAssetData> {
        self.assets.iter().filter_map(|(_, a)| match a {
            AppAsset::RegistryAsset(r) => Some(r),
            _ => None,
        })
    }

    /// Get an available signer identified by name.
    ///
    /// In some cases, like with a jade not currently linked, it may try to connect to it first
    pub fn get_available_signer(&mut self, name: &str) -> Result<&AnySigner, Error> {
        let timeout = Some(self.config.timeout);
        self.signers.get_available(name, timeout)
    }
}
