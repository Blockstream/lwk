use std::sync::{Mutex, PoisonError};

use elements::{bitcoin::bip32::ExtendedPubKey, pset::PartiallySignedTransaction};

use crate::{sign_pset, Jade};

#[derive(Debug)]
pub struct MutexJade(Mutex<Jade>);

impl MutexJade {
    pub fn new(jade: Jade) -> Self {
        Self(Mutex::new(jade))
    }
    pub fn sign_pset(
        &self,
        pset: &mut PartiallySignedTransaction,
    ) -> Result<u32, sign_pset::Error> {
        self.0.lock().unwrap().sign_pset(pset)
    }

    pub fn get_xpub(
        &self,
        params: crate::protocol::GetXpubParams,
    ) -> Result<ExtendedPubKey, crate::error::Error> {
        self.0.lock().unwrap().get_xpub(params)
    }

    pub fn unlock(&self) -> Result<(), crate::unlock::Error> {
        self.0.lock().unwrap().unlock() // TODO remove unwrap here and in the other methods
    }

    pub fn into_inner(self) -> Result<Jade, Box<PoisonError<Jade>>> {
        self.0.into_inner().map_err(Box::new)
    }

    pub fn get_mut(&mut self) -> Result<&mut Jade, Box<PoisonError<&mut Jade>>> {
        self.0.get_mut().map_err(Box::new)
    }

    pub fn register_multisig(&self, params: crate::register_multisig::RegisterMultisigParams) {
        self.0.lock().unwrap().register_multisig(params).unwrap();
    }
}
