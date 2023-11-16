use std::sync::{Mutex, PoisonError};

use common::Signer;
use elements::{bitcoin::bip32::ExtendedPubKey, pset::PartiallySignedTransaction};

use crate::consts::{BAUD_RATE, TIMEOUT};
use crate::network::Network;
use crate::{sign_pset, Jade};

#[derive(Debug)]
pub struct MutexJade(Mutex<Jade>);

impl MutexJade {
    pub fn new(jade: Jade) -> Self {
        Self(Mutex::new(jade))
    }

    #[cfg(feature = "serial")]
    pub fn from_serial(network: Network) -> Result<Self, crate::error::Error> {
        let ports = serialport::available_ports()?;
        if ports.is_empty() {
            Err(crate::error::Error::NoAvailablePorts)
        } else {
            // TODO: only one serial jade supported
            let path = &ports[0].port_name;
            let port = serialport::new(path, BAUD_RATE).timeout(TIMEOUT).open()?;
            Ok(Self::new(Jade::new(port.into(), network)))
        }
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

impl Signer for MutexJade {
    type Error = crate::sign_pset::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Ok(self.sign_pset(pset)?)
    }
}

impl Signer for &MutexJade {
    type Error = crate::sign_pset::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Ok(self.sign_pset(pset)?)
    }
}
