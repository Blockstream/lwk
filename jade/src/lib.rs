#![doc = include_str!("../README.md")]

use std::{
    collections::HashMap,
    io::ErrorKind,
    time::{SystemTime, UNIX_EPOCH},
};

use connection::Connection;
use elements::bitcoin::bip32::{DerivationPath, ExtendedPubKey, Fingerprint};
use get_receive_address::GetReceiveAddressParams;
use protocol::{
    AuthResult, AuthUserParams, DebugSetMnemonicParams, EntropyParams, EpochParams,
    GetSignatureParams, GetXpubParams, HandshakeData, HandshakeInitParams, IsAuthResult, Params,
    RegisteredMultisig, Request, Response, SignMessageParams, UpdatePinserverParams,
    VersionInfoResult,
};
use rand::RngCore;
use register_multisig::RegisterMultisigParams;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use sign_liquid_tx::{SignLiquidTxParams, TxInputParams};

pub mod connection;
pub mod consts;
pub mod error;
pub mod get_receive_address;
pub mod mutex_jade;
mod network;
pub mod protocol;
pub mod register_multisig;
pub mod sign_liquid_tx;
pub mod sign_pset;
pub mod unlock;

pub use consts::{BAUD_RATE, TIMEOUT};
pub use error::Error;
pub use network::Network;

#[cfg(feature = "serial")]
pub use serialport;

pub type Result<T> = std::result::Result<T, error::Error>;

#[derive(Debug)]
pub struct Jade {
    /// Jade working via emulator(tcp), physical(serial/bluetooth)
    conn: Connection,

    /// The network
    network: crate::Network,

    /// Cached master xpub
    master_xpub: Option<ExtendedPubKey>,
}

#[cfg(feature = "serial")]
#[derive(Debug)]
pub struct SerialJade {
    pub jade: Jade,
    pub network: Network,
    pub path: String,
    pub version: VersionInfoResult,
}

impl Jade {
    pub fn new(conn: Connection, network: Network) -> Self {
        Self {
            conn,
            network,
            master_xpub: None,
        }
    }

    #[cfg(feature = "serial")]
    pub fn scan_serial() -> Result<Option<SerialJade>> {
        let ports = serialport::available_ports()?;
        if ports.is_empty() {
            Ok(None)
        } else {
            // todo: loop through ports?
            let path = ports[0].port_name.clone();
            let port = serialport::new(&path, BAUD_RATE).timeout(TIMEOUT).open()?;

            let network = Network::TestnetLiquid; // todo: network selection
            let mut jade = Jade::new(port.into(), network);
            let version = jade.version_info()?;

            Ok(Some(SerialJade {
                jade,
                network,
                path,
                version,
            }))
        }
    }

    fn check_network(&self, passed: Network) -> Result<()> {
        let init = self.network;
        if passed != init {
            Err(Error::MismatchingXpub { init, passed })
        } else {
            Ok(())
        }
    }

    pub fn ping(&mut self) -> Result<u8> {
        self.send_request("ping", None)
    }

    pub fn logout(&mut self) -> Result<bool> {
        self.send_request("logout", None)
    }

    pub fn version_info(&mut self) -> Result<VersionInfoResult> {
        self.send_request("get_version_info", None)
    }

    pub fn set_epoch(&mut self, epoch: u64) -> Result<bool> {
        let params = Params::Epoch(EpochParams { epoch });
        self.send_request("set_epoch", Some(params))
    }

    pub fn add_entropy(&mut self, entropy: &[u8]) -> Result<bool> {
        let params = Params::Entropy(EntropyParams {
            entropy: entropy.to_vec(),
        });
        self.send_request("add_entropy", Some(params))
    }

    pub fn auth_user(&mut self) -> Result<IsAuthResult<String>> {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error::Error::SystemTimeError)?
            .as_secs();
        let params = Params::AuthUser(AuthUserParams {
            network: self.network,
            epoch,
        });
        self.send_request("auth_user", Some(params))
    }

    pub fn handshake_init(
        &mut self,
        params: HandshakeInitParams,
    ) -> Result<AuthResult<HandshakeData>> {
        let params = Params::HandshakeInit(params);
        self.send_request("handshake_init", Some(params))
    }

    pub fn update_pinserver(&mut self, params: UpdatePinserverParams) -> Result<bool> {
        let params = Params::UpdatePinServer(params);
        self.send_request("update_pinserver", Some(params))
    }

    pub fn handshake_complete(
        &mut self,
        params: protocol::HandshakeCompleteParams,
    ) -> Result<bool> {
        let params = Params::HandshakeComplete(params);
        self.send_request("handshake_complete", Some(params))
    }

    fn inner_get_xpub(&mut self, params: GetXpubParams) -> Result<ExtendedPubKey> {
        self.check_network(params.network)?;
        let params = Params::GetXpub(params);
        self.send_request("get_xpub", Some(params))
    }

    pub fn get_xpub(&mut self, params: GetXpubParams) -> Result<ExtendedPubKey> {
        if params.path.is_empty() {
            self.get_master_xpub()
        } else {
            self.inner_get_xpub(params)
        }
    }

    pub fn fingerprint(&mut self) -> Result<Fingerprint> {
        Ok(self.get_master_xpub()?.fingerprint())
    }

    pub fn get_master_xpub(&mut self) -> Result<ExtendedPubKey> {
        if self.master_xpub.is_none() {
            let master_xpub = self.inner_get_xpub(GetXpubParams {
                network: self.network,
                path: vec![],
            })?;
            self.master_xpub = Some(master_xpub);
        }
        Ok(self.master_xpub.expect("ensure it is some before"))
    }

    pub fn get_receive_address(&mut self, params: GetReceiveAddressParams) -> Result<String> {
        self.check_network(params.network)?;
        let params = Params::GetReceiveAddress(params);
        self.send_request("get_receive_address", Some(params))
    }

    pub fn sign_message(&mut self, params: SignMessageParams) -> Result<ByteBuf> {
        let params = Params::SignMessage(params);
        self.send_request("sign_message", Some(params))
    }

    pub fn get_signature_for_msg(&mut self, params: GetSignatureParams) -> Result<String> {
        let params = Params::GetSignature(params);
        self.send_request("get_signature", Some(params))
    }

    pub fn get_signature_for_tx(&mut self, params: GetSignatureParams) -> Result<ByteBuf> {
        let params = Params::GetSignature(params);
        self.send_request("get_signature", Some(params))
    }

    pub fn sign_liquid_tx(&mut self, params: SignLiquidTxParams) -> Result<bool> {
        self.check_network(params.network)?;
        let params = Params::SignLiquidTx(params);
        self.send_request("sign_liquid_tx", Some(params))
    }

    pub fn tx_input(&mut self, params: TxInputParams) -> Result<ByteBuf> {
        let params = Params::TxInput(params);
        self.send_request("tx_input", Some(params))
    }

    pub fn debug_set_mnemonic(&mut self, params: DebugSetMnemonicParams) -> Result<bool> {
        let params = Params::DebugSetMnemonic(params);
        self.send_request("debug_set_mnemonic", Some(params))
    }

    pub fn register_multisig(&mut self, params: RegisterMultisigParams) -> Result<bool> {
        self.check_network(params.network)?;
        let params = Params::RegisterMultisig(params);
        self.send_request("register_multisig", Some(params))
    }

    pub fn get_registered_multisigs(&mut self) -> Result<HashMap<String, RegisteredMultisig>> {
        self.send_request("get_registered_multisigs", None)
    }

    fn send_request<T>(&mut self, method: &str, params: Option<Params>) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        let mut rng = rand::thread_rng();
        let id = rng.next_u32().to_string();
        let req = Request {
            id,
            method: method.into(),
            params,
        };
        let mut buf = Vec::new();
        ciborium::into_writer(&req, &mut buf)?;
        tracing::debug!(
            "\n--->\t{:#?}\n\t({} bytes) {}",
            &req,
            buf.len(),
            &hex::encode(&buf),
        );

        self.conn.write_all(&buf).map_err(|e| Error::IoError(e))?; // not sure why the map_err is needed

        let mut rx = [0u8; 4096];

        let mut total = 0;
        loop {
            match self.conn.read(&mut rx[total..]) {
                Ok(len) => {
                    total += len;
                    match ciborium::from_reader::<Response<T>, &[u8]>(&rx[..total]) {
                        Ok(r) => {
                            if let Some(result) = r.result {
                                tracing::debug!(
                                    "\n<---\t{:?}\n\t({} bytes) {}",
                                    &result,
                                    total,
                                    hex::encode(&rx[..total])
                                );
                                return Ok(result);
                            }
                            if let Some(error) = r.error {
                                return Err(Error::JadeError(error));
                            }
                            return Err(Error::JadeNeitherErrorNorResult);
                        }

                        Err(e) => {
                            let res = ciborium::from_reader::<ciborium::Value, &[u8]>(&rx[..total]);
                            if let Ok(value) = res {
                                // The value returned is a valid CBOR, but our structs doesn't map it correctly
                                dbg!(&value);
                                return Err(Error::Des(e));
                            }

                            if len == 0 {
                                // There is no more data coming from jade and we can't parse its message, return error
                                return Err(Error::Des(e));
                            } else {
                                // it may be the parsing failed because there is other data to be read
                            }
                        }
                    }
                }
                Err(e) => {
                    if e.kind() != ErrorKind::Interrupted {
                        dbg!(&e);
                        return Err(Error::IoError(e));
                    }
                }
            }
        }
    }
}

pub fn derivation_path_to_vec(path: &DerivationPath) -> Vec<u32> {
    path.into_iter().map(|e| (*e).into()).collect()
}
