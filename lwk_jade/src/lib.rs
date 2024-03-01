#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use std::{
    collections::HashMap,
    io::ErrorKind,
};

use connection::Connection;
use elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};
use get_receive_address::GetReceiveAddressParams;
use protocol::{
    AuthResult, AuthUserParams, DebugSetMnemonicParams, EntropyParams, EpochParams, FullRequest,
    GetMasterBlindingKeyParams, GetSignatureParams, GetXpubParams, HandshakeCompleteParams,
    HandshakeData, HandshakeInitParams, IsAuthResult, Request, Response, SignMessageParams,
    UpdatePinserverParams, VersionInfoResult,
};
use rand::RngCore;
use register_multisig::{
    GetRegisteredMultisigParams, RegisterMultisigParams, RegisteredMultisig,
    RegisteredMultisigDetails,
};
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
    master_xpub: Option<Xpub>,
}

impl Jade {
    pub fn new(conn: Connection, network: Network) -> Self {
        Self {
            conn,
            network,
            master_xpub: None,
        }
    }

    pub fn ping(&mut self) -> Result<u8> {
        self.send(Request::Ping)
    }

    pub fn logout(&mut self) -> Result<bool> {
        self.send(Request::Logout)
    }

    pub fn version_info(&mut self) -> Result<VersionInfoResult> {
        self.send(Request::GetVersionInfo)
    }

    pub fn set_epoch(&mut self, epoch: u64) -> Result<bool> {
        self.send(Request::SetEpoch(EpochParams { epoch }))
    }

    pub fn add_entropy(&mut self, entropy: Vec<u8>) -> Result<bool> {
        self.send(Request::AddEntropy(EntropyParams { entropy }))
    }

    pub fn auth_user(&mut self, params: AuthUserParams) -> Result<IsAuthResult<String>> {
        self.send(Request::AuthUser(params))
    }

    pub fn handshake_init(
        &mut self,
        params: HandshakeInitParams,
    ) -> Result<AuthResult<HandshakeData>> {
        self.send(Request::HandshakeInit(params))
    }

    pub fn update_pinserver(&mut self, params: UpdatePinserverParams) -> Result<bool> {
        self.send(Request::UpdatePinserver(params))
    }

    pub fn handshake_complete(&mut self, params: HandshakeCompleteParams) -> Result<bool> {
        self.send(Request::HandshakeComplete(params))
    }

    fn get_xpub(&mut self, params: GetXpubParams) -> Result<Xpub> {
        self.send(Request::GetXpub(params))
    }

    pub fn get_receive_address(&mut self, params: GetReceiveAddressParams) -> Result<String> {
        self.send(Request::GetReceiveAddress(params))
    }

    pub fn get_master_blinding_key(
        &mut self,
        params: GetMasterBlindingKeyParams,
    ) -> Result<ByteBuf> {
        self.send(Request::GetMasterBlindingKey(params))
    }

    pub fn sign_message(&mut self, params: SignMessageParams) -> Result<ByteBuf> {
        self.send(Request::SignMessage(params))
    }

    pub fn get_signature_for_msg(&mut self, params: GetSignatureParams) -> Result<String> {
        self.send(Request::GetSignature(params))
    }

    pub fn get_signature_for_tx(&mut self, params: GetSignatureParams) -> Result<ByteBuf> {
        self.send(Request::GetSignature(params))
    }

    pub fn sign_liquid_tx(&mut self, params: SignLiquidTxParams) -> Result<bool> {
        self.send(Request::SignLiquidTx(params))
    }

    pub fn tx_input(&mut self, params: TxInputParams) -> Result<ByteBuf> {
        self.send(Request::TxInput(params))
    }

    pub fn debug_set_mnemonic(&mut self, params: DebugSetMnemonicParams) -> Result<bool> {
        self.send(Request::DebugSetMnemonic(params))
    }

    pub fn register_multisig(&mut self, params: RegisterMultisigParams) -> Result<bool> {
        self.send(Request::RegisterMultisig(params))
    }

    pub fn get_registered_multisigs(&mut self) -> Result<HashMap<String, RegisteredMultisig>> {
        self.send(Request::GetRegisteredMultisigs)
    }

    pub fn get_registered_multisig(
        &mut self,
        params: GetRegisteredMultisigParams,
    ) -> Result<RegisteredMultisigDetails> {
        self.send(Request::GetRegisteredMultisig(params))
    }

    pub fn get_xpub_cached(&mut self, params: GetXpubParams) -> Result<Xpub> {
        if params.path.is_empty() {
            self.get_master_xpub()
        } else {
            self.get_xpub(params)
        }
    }

    pub fn fingerprint(&mut self) -> Result<Fingerprint> {
        Ok(self.get_master_xpub()?.fingerprint())
    }

    fn check_network(&self, passed: Network) -> Result<()> {
        let init = self.network;
        if passed != init {
            Err(Error::MismatchingXpub { init, passed })
        } else {
            Ok(())
        }
    }

    pub fn get_master_xpub(&mut self) -> Result<Xpub> {
        if self.master_xpub.is_none() {
            let master_xpub = self.get_xpub(GetXpubParams {
                network: self.network,
                path: vec![],
            })?;
            self.master_xpub = Some(master_xpub);
        }
        Ok(self.master_xpub.expect("ensure it is some before"))
    }

    fn send<T>(&mut self, request: Request) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        if let Some(network) = request.network() {
            self.check_network(network)?;
        }
        let mut rng = rand::thread_rng();
        let id = rng.next_u32().to_string();
        let req = FullRequest {
            id,
            method: request.to_string(),
            params: request,
        };
        let mut buf = Vec::new();
        serde_cbor::to_writer(&mut buf, &req)?;
        tracing::debug!(
            "\n--->\t{:#?}\n\t({} bytes) {}",
            &req,
            buf.len(),
            &hex::encode(&buf),
        );

        self.conn.write_all(&buf)?;

        let mut rx = [0u8; 4096];

        let mut total = 0;
        loop {
            match self.conn.read(&mut rx[total..]) {
                Ok(len) => {
                    total += len;
                    match serde_cbor::from_reader::<Response<T>, &[u8]>(&rx[..total]) {
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
                            let res =
                                serde_cbor::from_reader::<serde_cbor::Value, &[u8]>(&rx[..total]);
                            if let Ok(value) = res {
                                // The value returned is a valid CBOR, but our structs doesn't map it correctly
                                dbg!(&value);
                                return Err(Error::SerdeCbor(e));
                            }

                            if len == 0 {
                                // There is no more data coming from jade and we can't parse its message, return error
                                return Err(Error::SerdeCbor(e));
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
