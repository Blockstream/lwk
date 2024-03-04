use std::{collections::HashMap, io::ErrorKind};

use crate::connection::Connection;
use crate::get_receive_address::GetReceiveAddressParams;
use crate::protocol::{
    AuthResult, AuthUserParams, DebugSetMnemonicParams, EntropyParams, EpochParams,
    GetMasterBlindingKeyParams, GetSignatureParams, GetXpubParams, HandshakeCompleteParams,
    HandshakeData, HandshakeInitParams, IsAuthResult, Request, SignMessageParams,
    UpdatePinserverParams, VersionInfoResult,
};
use crate::register_multisig::{
    GetRegisteredMultisigParams, RegisterMultisigParams, RegisteredMultisig,
    RegisteredMultisigDetails,
};
use crate::sign_liquid_tx::{SignLiquidTxParams, TxInputParams};
use crate::{try_parse_response, vec_to_derivation_path, Error, Network, Result};
use elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;

#[derive(Debug)]
pub struct Jade {
    /// Jade working via emulator(tcp), physical(serial/bluetooth)
    conn: Connection,

    /// The network
    pub(crate) network: crate::Network,

    /// Cached xpubs
    cached_xpubs: HashMap<DerivationPath, Xpub>,
}

impl Jade {
    pub fn new(conn: Connection, network: Network) -> Self {
        Self {
            conn,
            network,
            cached_xpubs: HashMap::new(),
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
        if let Some(xpub) = self.cached_xpubs.get(&vec_to_derivation_path(&params.path)) {
            Ok(*xpub)
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
        let params = GetXpubParams {
            network: self.network,
            path: vec![],
        };
        self.get_xpub_cached(params)
    }

    /// Unlock an already initialized Jade.
    ///
    /// The device asks for the pin,
    /// and the host performs network calls to the pin server
    /// to decrypt the secret on the device.
    pub fn unlock(&mut self) -> Result<()> {
        match self.auth_user(AuthUserParams::new(self.network)?)? {
            IsAuthResult::AlreadyAuth(result) => {
                if result {
                    Ok(())
                } else {
                    // Jade is not setup, and the user declined to do it on Jade screen
                    Err(Error::NotInitialized)
                }
            }
            IsAuthResult::AuthResult(result) => {
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let resp = minreq::post(url).send()?;
                if resp.status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), resp.status_code));
                }

                let params: HandshakeInitParams = serde_json::from_slice(resp.as_bytes())?;
                let result = self.handshake_init(params)?;
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let data = serde_json::to_vec(result.data())?;
                let resp = minreq::post(url).with_body(data).send()?;
                if resp.status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), resp.status_code));
                }
                let params: HandshakeCompleteParams = serde_json::from_slice(resp.as_bytes())?;

                let result = self.handshake_complete(params)?;

                if !result {
                    return Err(Error::HandshakeFailed);
                }

                Ok(())
            }
        }
    }

    pub(crate) fn send<T>(&mut self, request: Request) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        if let Some(network) = request.network() {
            self.check_network(network)?;
        }
        let buf = request.serialize()?;

        self.conn.write_all(&buf)?;

        let mut rx = [0u8; 4096];

        let mut total = 0;
        loop {
            match self.conn.read(&mut rx[total..]) {
                Ok(len) => {
                    total += len;
                    let reader = &rx[..total];

                    if let Some(value) = try_parse_response(reader) {
                        return value;
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
