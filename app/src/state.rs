use std::collections::HashMap;

use common::Signer;
use signer::AnySigner;
use wollet::bitcoin::bip32::Fingerprint;
use wollet::elements::{AssetId, OutPoint, Txid};
use wollet::Contract;
use wollet::Wollet;

use crate::config::Config;
use crate::Error;

pub enum AppSigner {
    AvailableSigner(AnySigner),
    ExternalSigner(Fingerprint),
}

impl AppSigner {
    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            AppSigner::AvailableSigner(s) => s.fingerprint().unwrap(), // TODO
            AppSigner::ExternalSigner(f) => *f,
        }
    }
}

#[allow(dead_code)]
pub enum AppAsset {
    /// The policy asset (L-BTC)
    PolicyAsset,

    /// An asset with contract committed to it
    RegistryAsset(Contract),

    /// A reissuance token for an asset
    ReissuanceToken(AssetId),
}

impl AppAsset {
    pub fn name(&self) -> String {
        match self {
            AppAsset::PolicyAsset => "liquid bitcoin".into(),
            AppAsset::RegistryAsset(contract) => contract.name.clone(),
            AppAsset::ReissuanceToken(asset_id) => format!("reissuance token for {asset_id}"),
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

    pub fn get_available(&self, name: &str) -> Result<&AnySigner, Error> {
        match self.get(name)? {
            AppSigner::AvailableSigner(signer) => Ok(signer),
            AppSigner::ExternalSigner(_) => Err(Error::Generic(
                "Invalid operation for external signer".to_string(),
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
        self.assets.0.insert(asset_id, AppAsset::PolicyAsset);
    }

    #[allow(dead_code)]
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
        self.assets
            .0
            .insert(asset_id, AppAsset::RegistryAsset(contract));
        self.assets
            .0
            .insert(token_id, AppAsset::ReissuanceToken(asset_id_c));
        Ok(())
    }
}
