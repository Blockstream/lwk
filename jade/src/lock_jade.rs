use std::sync::Mutex;

use elements::{bitcoin::bip32::ExtendedPubKey, pset::PartiallySignedTransaction};

use crate::{sign_pset, Jade};

#[derive(Debug)]
pub struct LockJade(Mutex<Jade>);

impl LockJade {
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
}
