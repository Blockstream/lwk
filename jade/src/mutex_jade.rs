use std::sync::Mutex;

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

    pub fn unlock_jade(&self) -> Result<bool, crate::unlock_jade::Error> {
        self.0.lock().unwrap().unlock_jade() // TODO remove unwrap here and in the other methods
    }
}
