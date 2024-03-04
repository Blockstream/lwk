use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};

use elements::{bitcoin::bip32::Xpub, pset::PartiallySignedTransaction};
use elements_miniscript::slip77::{self, MasterBlindingKey};
use lwk_common::Signer;

use crate::connection::Connection;
use crate::get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant};
use crate::network::Network;
use crate::protocol::{GetMasterBlindingKeyParams, GetXpubParams};
use crate::{derivation_path_to_vec, Error, Jade};

#[cfg(feature = "serial")]
use crate::consts::{BAUD_RATE, TIMEOUT};

#[derive(Debug)]
pub struct MutexJade {
    inner: Mutex<Jade>,
    network: Network,
}

// Taken from reference impl https://github.com/Blockstream/Jade/blob/f7fc4de8c3662b082c7d41e9354c4ff573f371ff/jadepy/jade_serial.py#L24
#[cfg(feature = "serial")]
const JADE_DEVICE_IDS: [(u16, u16); 4] = [
    (0x10c4, 0xea60),
    (0x1a86, 0x55d4),
    (0x0403, 0x6001),
    (0x1a86, 0x7523),
];

impl MutexJade {
    pub fn new(jade: Jade) -> Self {
        let network = jade.network;
        Self {
            inner: Mutex::new(jade),
            network,
        }
    }

    #[cfg(feature = "serial")]
    pub fn from_serial(
        network: Network,
        port_name: &str,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self, Error> {
        tracing::info!("serial port {port_name}");
        let timeout = timeout.unwrap_or(TIMEOUT);
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(timeout)
            .open()?;
        Ok(Self::new(Jade::new(port.into(), network)))
    }

    #[cfg(feature = "serial")]
    /// Try to unlock a jade on any available serial port, returning all of the attempts
    pub fn from_any_serial(
        network: Network,
        timeout: Option<std::time::Duration>,
    ) -> Vec<Result<Self, Error>> {
        let mut result = vec![];
        let ports = Self::available_ports_with_jade();
        tracing::debug!("available serial ports possibly with jade: {}", ports.len());
        for port in ports {
            let jade_res = Self::from_serial(network, &port.port_name, timeout);
            tracing::debug!("trying: {port:?} return {jade_res:?}");

            // TODO green_qt calls also get_version_info
            result.push(jade_res);
        }
        result
    }

    #[cfg(feature = "serial")]
    pub fn available_ports_with_jade() -> Vec<serialport::SerialPortInfo> {
        let ports = serialport::available_ports().unwrap_or_default();
        tracing::debug!("available serial ports: {}", ports.len());

        ports
            .into_iter()
            .filter(|e| {
                if let serialport::SerialPortType::UsbPort(val) = &e.port_type {
                    JADE_DEVICE_IDS.contains(&(val.vid, val.pid))
                } else {
                    false
                }
            })
            .collect()
    }

    #[cfg(feature = "serial")]
    pub fn from_serial_matching_id(
        network: Network,
        id: &elements::bitcoin::XKeyIdentifier,
        timeout: Option<std::time::Duration>,
    ) -> Option<Self> {
        Self::from_any_serial(network, timeout)
            .into_iter()
            .filter_map(|e| e.ok())
            .find(|e| {
                if let Ok(c) = e.identifier() {
                    &c == id
                } else {
                    false
                }
            })
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

    pub fn get_receive_address_single(
        &self,
        variant: Variant,
        path: Vec<u32>,
    ) -> Result<String, Error> {
        let params = GetReceiveAddressParams {
            network: self.network(),
            address: SingleOrMulti::Single { variant, path },
        };
        self.inner.lock()?.get_receive_address(params)
    }

    pub fn get_receive_address_multi(
        &self,
        name: &str,
        paths: Vec<Vec<u32>>,
    ) -> Result<String, Error> {
        let params = GetReceiveAddressParams {
            network: self.network(),
            address: SingleOrMulti::Multi {
                multisig_name: name.to_string(),
                paths,
            },
        };
        self.inner.lock()?.get_receive_address(params)
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
    ) -> Result<Xpub, Self::Error> {
        let network = self.network();
        let params = GetXpubParams {
            network,
            path: derivation_path_to_vec(path),
        };

        self.unlock()?;
        self.inner.lock()?.get_cached_xpub(params)
    }

    fn slip77_master_blinding_key(&self) -> Result<slip77::MasterBlindingKey, Self::Error> {
        let params = GetMasterBlindingKeyParams {
            only_if_silent: false,
        };
        let bytes = self.inner.lock()?.get_master_blinding_key(params)?;
        let array: [u8; 32] = bytes
            .to_vec()
            .try_into()
            .map_err(|_| Self::Error::Slip77MasterBlindingKeyInvalidSize)?;
        Ok(slip77::MasterBlindingKey::from(array))
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
    ) -> Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }
}
