use std::collections::HashMap;

use common::Signer;
use signer::AnySigner;
use wollet::bitcoin::bip32::Fingerprint;
use wollet::Wollet;

use crate::config::Config;
use crate::{Error, Result};

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

#[derive(Default)]
pub struct Wollets(HashMap<String, Wollet>);

#[derive(Default)]
pub struct Signers(HashMap<String, AppSigner>);

#[derive(Default)]
pub struct State {
    // TODO: config is read-only, so it's not useful to wrap it in a mutex.
    // Ideally it should be in _another_ struct accessible by method_handler.
    pub config: Config,
    pub wollets: Wollets,
    pub signers: Signers,
}

impl Wollets {
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Result<&Wollet> {
        self.0
            .get(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }

    pub fn get_mut(&mut self, name: &str) -> Result<&mut Wollet> {
        self.0
            .get_mut(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }

    pub fn insert(&mut self, name: &str, wollet: Wollet) -> Result<()> {
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

    pub fn remove(&mut self, name: &str) -> Result<Wollet> {
        self.0
            .remove(name)
            .ok_or_else(|| Error::WalletNotExist(name.to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Wollet)> {
        self.0.iter()
    }
}

impl Signers {
    pub fn get(&self, name: &str) -> Result<&AppSigner> {
        self.0
            .get(name)
            .ok_or_else(|| Error::SignerNotExist(name.to_string()))
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, name: &str) -> Result<&mut AppSigner> {
        self.0
            .get_mut(name)
            .ok_or_else(|| Error::SignerNotExist(name.to_string()))
    }

    pub fn get_available(&self, name: &str) -> Result<&AnySigner> {
        match self.get(name)? {
            AppSigner::AvailableSigner(signer) => Ok(signer),
            AppSigner::ExternalSigner(_) => Err(Error::Generic(
                "Invalid operation for external signer".to_string(),
            )),
        }
    }

    pub fn insert(&mut self, name: &str, signer: AppSigner) -> Result<()> {
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

    pub fn remove(&mut self, name: &str) -> Result<AppSigner> {
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
