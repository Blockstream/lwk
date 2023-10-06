use std::sync::Mutex;

use elements::pset::PartiallySignedTransaction;

use crate::{sign_pset::Error, Jade};

pub struct LockJade(Mutex<Jade>);

impl LockJade {
    pub fn sign_pset(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Error> {
        self.0.lock().unwrap().sign_pset(pset)
    }
}
