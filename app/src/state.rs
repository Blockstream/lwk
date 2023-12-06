use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::str::FromStr;

use common::Signer;
use jade::mutex_jade::MutexJade;
use jade::Network;
use serde::Serialize;
use signer::AnySigner;
use wollet::bitcoin::bip32::Fingerprint;
use wollet::bitcoin::hash_types::XpubIdentifier;
use wollet::elements::pset::elip100::AssetMetadata;
use wollet::elements::{AssetId, OutPoint, Txid};
use wollet::Contract;
use wollet::Wollet;

use crate::config::Config;
use crate::Error;

#[derive(Debug)]
pub enum AppSigner {
    JadeId(XpubIdentifier, Network),
    AvailableSigner(AnySigner),
    ExternalSigner(Fingerprint),
}

impl AppSigner {
    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            AppSigner::AvailableSigner(s) => s.fingerprint().unwrap(), // TODO
            AppSigner::ExternalSigner(f) => *f,
            AppSigner::JadeId(id, _) => id_to_fingerprint(id),
        }
    }
}

// TODO upstream as method of XKeyIdentifier to rust-bitcoin
pub fn id_to_fingerprint(id: &XpubIdentifier) -> Fingerprint {
    id[0..4].try_into().expect("4 is the fingerprint length")
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RegistryAssetData {
    asset_id: AssetId,
    token_id: AssetId,
    issuance_prevout: OutPoint,
    issuance_is_confidential: bool,
    contract: Contract,
}

impl RegistryAssetData {
    pub fn contract_str(&self) -> String {
        serde_json::to_string(&self.contract).expect("contract")
    }
}

pub enum AppAsset {
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
            AppAsset::RegistryAsset(d) => d.contract.name.clone(),
            AppAsset::ReissuanceToken(d) => {
                format!("reissuance token for {}", d.contract.name)
            }
        }
    }

    pub fn ticker(&self) -> String {
        match self {
            AppAsset::PolicyAsset(_) => "L-BTC".into(),
            AppAsset::RegistryAsset(d) => d.contract.ticker.clone(),
            AppAsset::ReissuanceToken(d) => {
                format!("reissuance token for {}", d.contract.ticker)
            }
        }
    }

    #[allow(dead_code)]
    pub fn asset_metadata(&self) -> Option<AssetMetadata> {
        match self {
            AppAsset::PolicyAsset(_) => None,
            AppAsset::RegistryAsset(d) => {
                Some(AssetMetadata::new(d.contract_str(), d.issuance_prevout))
            }
            AppAsset::ReissuanceToken(d) => {
                Some(AssetMetadata::new(d.contract_str(), d.issuance_prevout))
            }
        }
    }

    pub fn asset_id(&self) -> AssetId {
        match self {
            AppAsset::PolicyAsset(asset) => *asset,
            AppAsset::RegistryAsset(d) => d.asset_id,
            AppAsset::ReissuanceToken(d) => d.token_id,
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
pub struct State {
    // TODO: config is read-only, so it's not useful to wrap it in a mutex.
    // Ideally it should be in _another_ struct accessible by method_handler.
    pub config: Config,
    pub wollets: Wollets,
    pub signers: Signers,
    pub assets: Assets,
    pub do_persist: bool,
}

impl Wollets {
    #[allow(dead_code)]
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

        let a = |w: &Wollet| w.address(Some(0)).unwrap().address().to_string();

        let vec: Vec<_> = self
            .0
            .iter()
            .filter(|(_, w)| a(w) == a(&wollet))
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
    pub fn get_available(&mut self, name: &str) -> Result<&AnySigner, Error> {
        let app_signer = self.get(name)?;
        tracing::debug!("get_available({}) return {:?}", name, app_signer);
        let jade = match app_signer {
            AppSigner::JadeId(id, network) => {
                // try to connect JadeId -> AvailableSigner(Jade)
                let jade = MutexJade::from_serial(*network)?;
                jade.unlock()?;
                let jade_id = jade.identifier()?;
                if id != &jade_id {
                    return Err(Error::Generic(format!(
                        "Connected jade identifier id:{} doesn't match with loaded signer {} id:{}",
                        jade_id, name, id
                    )));
                }
                Some(AppSigner::AvailableSigner(AnySigner::Jade(jade, *id)))
            }
            AppSigner::AvailableSigner(AnySigner::Jade(j, id)) => {
                // verify connection, if fails AvailableSigner(Jade) -> JadeId
                if j.unlock().is_err() {
                    // TODO ensure identifier it's cached
                    Some(AppSigner::JadeId(*id, j.network()))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(signer) = jade {
            // replace the existing AppSigner::JadeId with AppSigner::AvailableSigner
            self.0.insert(name.to_string(), signer);
        }

        match self.get(name)? {
            AppSigner::AvailableSigner(signer) => Ok(signer),
            AppSigner::ExternalSigner(_) => Err(Error::Generic(
                "Invalid operation for external signer".to_string(),
            )),
            AppSigner::JadeId(_, _) => Err(Error::Generic(
                "Invalid operation jade is not connected".to_string(),
            )),
        }
    }

    pub fn insert(&mut self, name: &str, signer: AppSigner) -> Result<(), Error> {
        if self.0.contains_key(name) {
            return Err(Error::SignerAlreadyLoaded(name.to_string()));
        }

        // TODO: matchin for fingerprint is not ideal, we could have collisions
        let vec: Vec<_> = self
            .0
            .iter()
            .filter(|(_, s)| s.fingerprint() == signer.fingerprint())
            .map(|(n, _)| n)
            .collect();
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

    /// Get a name from the fingerprint
    pub fn name_from_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        warnings: &mut Vec<String>,
    ) -> Option<String> {
        let names: Vec<_> = self
            .iter()
            .filter(|(_, s)| &s.fingerprint() == fingerprint)
            .map(|(n, _)| n.clone())
            .collect();
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
        prev_txid: Txid,
        prev_vout: u32,
        contract: Contract,
        is_confidential: Option<bool>,
    ) -> Result<(), Error> {
        let previous_output = OutPoint::new(prev_txid, prev_vout);
        let is_confidential = is_confidential.unwrap_or(false);
        let (asset_id_c, token_id) =
            wollet::issuance_ids(&contract, previous_output, is_confidential)?;
        if asset_id != asset_id_c {
            return Err(Error::InvalidContractForAsset(asset_id.to_string()));
        }
        let data = RegistryAssetData {
            asset_id,
            token_id,
            issuance_prevout: previous_output,
            issuance_is_confidential: is_confidential,
            contract,
        };
        self.assets
            .0
            .insert(asset_id, AppAsset::RegistryAsset(data.clone()));
        self.assets
            .0
            .insert(token_id, AppAsset::ReissuanceToken(data));
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
            let path = self.config.state_path();
            let mut file = OpenOptions::new()
                .create_new(!path.exists())
                .write(true)
                .append(true)
                .open(path)?;
            writeln!(file, "{}", data)?;
            file.sync_all()?;
        }
        Ok(())
    }
}
