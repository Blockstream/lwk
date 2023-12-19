use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};

use common::Signer;
use elements::{bitcoin::bip32::ExtendedPubKey, pset::PartiallySignedTransaction};
use elements_miniscript::slip77::MasterBlindingKey;
use rand::{thread_rng, Rng};

use crate::connection::Connection;
use crate::network::Network;
use crate::protocol::GetXpubParams;
use crate::{derivation_path_to_vec, Error, Jade};

#[cfg(feature = "serial")]
use crate::consts::{BAUD_RATE, TIMEOUT};

#[derive(Debug)]
pub struct MutexJade {
    inner: Mutex<Jade>,
    network: Network,
}

#[cfg(feature = "serial")]
fn enumerate_ports() -> Result<Vec<serialport::SerialPortInfo>, serialport::Error> {
    serialport::available_ports()
}

#[cfg(feature = "serial")]
pub fn first_port() -> Result<String, Error> {
    Ok(enumerate_ports()?
        .get(0)
        .ok_or(Error::NoAvailablePorts)?
        .port_name
        .to_string())
}

impl MutexJade {
    pub fn new(jade: Jade) -> Self {
        let network = jade.network;
        Self {
            inner: Mutex::new(jade),
            network,
        }
    }

    #[cfg(feature = "serial")]
    pub fn from_serial(network: Network, port_name: &str) -> Result<Self, Error> {
        tracing::info!("serial port {port_name}");
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(TIMEOUT)
            .open()?;
        Ok(Self::new(Jade::new(port.into(), network)))
    }

    #[cfg(feature = "serial")]
    /// Try to unlock a jade on any available serial port, returning all of the attempts
    pub fn from_any_serial(network: Network) -> Vec<Result<Self, Error>> {
        let mut result = vec![];
        for port in enumerate_ports().unwrap_or_default() {
            let jade_res = Self::from_serial(network, &port.port_name);
            result.push(jade_res);
        }
        result
    }

    pub fn from_socket(socket: SocketAddr, network: Network) -> Result<Self, Error> {
        let stream = std::net::TcpStream::connect(socket)?;
        let conn = Connection::TcpStream(stream);
        let jade = Jade::new(conn, network);
        Ok(Self::new(jade))
    }

    pub fn unlock(&self) -> Result<(), crate::Error> {
        self.inner.lock()?.unlock()
    }

    pub fn into_inner(self) -> Result<Jade, Box<PoisonError<Jade>>> {
        self.inner.into_inner().map_err(Box::new)
    }

    pub fn get_mut(&mut self) -> Result<&mut Jade, Box<PoisonError<&mut Jade>>> {
        self.inner.get_mut().map_err(Box::new)
    }

    pub fn register_multisig(
        &self,
        params: crate::register_multisig::RegisterMultisigParams,
    ) -> Result<(), Error> {
        self.unlock()?;
        self.inner.lock()?.register_multisig(params)?;
        Ok(())
    }

    pub fn network(&self) -> Network {
        self.network
    }
}

impl Signer for &MutexJade {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        self.unlock()?;
        self.inner.lock()?.sign(pset)
    }

    fn derive_xpub(
        &self,
        path: &elements::bitcoin::bip32::DerivationPath,
    ) -> Result<ExtendedPubKey, Self::Error> {
        let network = self.network();
        let params = GetXpubParams {
            network,
            path: derivation_path_to_vec(path),
        };

        self.unlock()?;
        self.inner.lock()?.get_xpub(params)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        // TODO ask jade instead of doing it randomly
        let mut bytes = [0u8; 32];
        thread_rng().fill(&mut bytes);
        Ok(MasterBlindingKey::from_seed(&bytes[..]))
    }
}

impl Signer for MutexJade {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(
        &self,
        path: &elements::bitcoin::bip32::DerivationPath,
    ) -> Result<ExtendedPubKey, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }
}
