use std::collections::BTreeMap;
use std::{collections::HashMap, io::ErrorKind};

use crate::get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant};
use crate::protocol::{
    AuthUserParams, DebugSetMnemonicParams, EntropyParams, EpochParams, GenericMethod,
    GetMasterBlindingKeyParams, GetSignatureParams, GetXpubParams, IsAuthResult, Request,
    SignMessageParams, UpdatePinserverParams, VersionInfoResult,
};
use crate::register_multisig::{
    GetRegisteredMultisigParams, RegisterMultisigParams, RegisteredMultisig,
    RegisteredMultisigDetails,
};
use crate::sign_liquid_tx::{SignLiquidTxParams, TxInputParams};
use crate::{json_to_cbor, try_parse_response, vec_to_derivation_path, Error, Result};
use elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};
use elements_miniscript::slip77;
use lwk_common::{Network, Stream};
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use tokio::sync::Mutex;

mod sign_pset;

#[derive(Debug)]
pub struct Jade<S: Stream> {
    /// Jade working via emulator(tcp), physical(serial/bluetooth)
    stream: S,

    /// The network
    network: Network,

    /// Cached master xpub
    cached_xpubs: Mutex<HashMap<DerivationPath, Xpub>>,

    /// Cached multisigs details
    multisigs_details: Mutex<Option<Vec<RegisteredMultisigDetails>>>,
}

/// Newtype wrapper for TcpStream to implement Stream trait
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct JadeTcpStream(pub Mutex<tokio::net::TcpStream>);

#[cfg(not(target_arch = "wasm32"))]
impl JadeTcpStream {
    pub fn new(stream: tokio::net::TcpStream) -> Self {
        Self(Mutex::new(stream))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Stream for JadeTcpStream {
    type Error = Error;

    async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        use tokio::io::AsyncReadExt;

        let mut stream = self.0.lock().await;
        Ok(stream.read(buf).await?)
    }

    async fn write(&self, data: &[u8]) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut stream = self.0.lock().await;
        Ok(stream.write_all(data).await?)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Jade<JadeTcpStream> {
    pub fn new_tcp(stream: tokio::net::TcpStream, network: Network) -> Self {
        Jade::new(JadeTcpStream::new(stream), network)
    }
}

impl<S: Stream<Error = Error>> Jade<S> {
    pub fn new(stream: S, network: Network) -> Self {
        Self {
            stream,
            network,
            cached_xpubs: Mutex::new(HashMap::new()),
            multisigs_details: Mutex::new(None),
        }
    }

    pub async fn generic(
        &self,
        method: String,
        params: serde_cbor::Value,
    ) -> Result<serde_cbor::Value> {
        self.send(Request::Generic(GenericMethod { method, params }))
            .await
    }

    pub async fn ping(&self) -> Result<u8> {
        self.send(Request::Ping).await
    }

    pub async fn logout(&self) -> Result<bool> {
        self.send(Request::Logout).await
    }

    pub async fn version_info(&self) -> Result<VersionInfoResult> {
        self.send(Request::GetVersionInfo).await
    }

    pub async fn set_epoch(&self, epoch: u64) -> Result<bool> {
        self.send(Request::SetEpoch(EpochParams { epoch })).await
    }

    pub async fn add_entropy(&self, entropy: Vec<u8>) -> Result<bool> {
        self.send(Request::AddEntropy(EntropyParams { entropy }))
            .await
    }

    pub async fn auth_user(&self, params: AuthUserParams) -> Result<IsAuthResult> {
        self.send(Request::AuthUser(params)).await
    }

    pub async fn update_pinserver(&self, params: UpdatePinserverParams) -> Result<bool> {
        self.send(Request::UpdatePinserver(params)).await
    }

    async fn get_xpub(&self, params: GetXpubParams) -> Result<Xpub> {
        self.send(Request::GetXpub(params)).await
    }

    pub async fn get_receive_address(&self, params: GetReceiveAddressParams) -> Result<String> {
        self.send(Request::GetReceiveAddress(params)).await
    }

    pub async fn get_master_blinding_key(
        &self,
        params: GetMasterBlindingKeyParams,
    ) -> Result<ByteBuf> {
        self.send(Request::GetMasterBlindingKey(params)).await
    }

    pub async fn sign_message_inner(&self, params: SignMessageParams) -> Result<ByteBuf> {
        self.send(Request::SignMessage(params)).await
    }

    pub async fn get_signature_for_msg(&self, params: GetSignatureParams) -> Result<String> {
        self.send(Request::GetSignature(params)).await
    }

    pub async fn get_signature_for_tx(&self, params: GetSignatureParams) -> Result<ByteBuf> {
        self.send(Request::GetSignature(params)).await
    }

    pub async fn sign_liquid_tx(&self, params: SignLiquidTxParams) -> Result<bool> {
        self.send(Request::SignLiquidTx(params)).await
    }

    pub async fn tx_input(&self, params: TxInputParams) -> Result<ByteBuf> {
        self.send(Request::TxInput(params)).await
    }

    pub async fn debug_set_mnemonic(&self, params: DebugSetMnemonicParams) -> Result<bool> {
        self.send(Request::DebugSetMnemonic(params)).await
    }

    pub async fn register_multisig(&self, params: RegisterMultisigParams) -> Result<bool> {
        self.invalidate_registered_multisigs().await;
        self.send(Request::RegisterMultisig(params)).await
    }

    pub async fn get_registered_multisigs(&self) -> Result<BTreeMap<String, RegisteredMultisig>> {
        self.send(Request::GetRegisteredMultisigs).await
    }

    pub async fn get_registered_multisig(
        &self,
        params: GetRegisteredMultisigParams,
    ) -> Result<RegisteredMultisigDetails> {
        self.send(Request::GetRegisteredMultisig(params)).await
    }

    pub async fn get_cached_xpub(&self, params: GetXpubParams) -> Result<Xpub> {
        let mut guard = self.cached_xpubs.lock().await;
        let der_path = vec_to_derivation_path(&params.path);
        if let Some(xpub) = guard.get(&der_path) {
            Ok(*xpub)
        } else {
            let result = self.get_xpub(params).await?;
            guard.insert(der_path, result);
            Ok(result)
        }
    }

    async fn get_cached_registered_multisigs(&self) -> Result<Vec<RegisteredMultisigDetails>> {
        let mut guard = self.multisigs_details.lock().await;
        if let Some(multisigs_details) = guard.as_ref() {
            Ok(multisigs_details.clone())
        } else {
            let result = self.ask_registered_multisigs().await?;
            *guard = Some(result.clone());
            Ok(result)
        }
    }

    async fn ask_registered_multisigs(&self) -> Result<Vec<RegisteredMultisigDetails>> {
        let mut multisigs_details = Vec::new();
        // Get all the registered multisigs including the signer
        for (name, _) in self.get_registered_multisigs().await? {
            let details = self
                .get_registered_multisig(GetRegisteredMultisigParams {
                    multisig_name: name,
                })
                .await?;
            multisigs_details.push(details);
        }
        Ok(multisigs_details)
    }

    async fn invalidate_registered_multisigs(&self) {
        *self.multisigs_details.lock().await = None;
    }

    pub async fn fingerprint(&self) -> Result<Fingerprint> {
        Ok(self.get_master_xpub().await?.fingerprint())
    }

    fn check_network(&self, passed: Network) -> Result<()> {
        let init = self.network;
        if passed != init {
            Err(Error::MismatchingXpub { init, passed })
        } else {
            Ok(())
        }
    }

    pub async fn get_master_xpub(&self) -> Result<Xpub> {
        let params = GetXpubParams {
            network: self.network,
            path: vec![],
        };
        self.get_cached_xpub(params).await
    }

    /// Unlock an already initialized Jade.
    ///
    /// The device asks for the pin,
    /// and the host performs network calls to the pin server
    /// to decrypt the secret on the device.
    pub async fn unlock(&self) -> Result<()> {
        match self.auth_user(AuthUserParams::new(self.network)).await? {
            IsAuthResult::AlreadyAuth(result) => {
                if result {
                    Ok(())
                } else {
                    // Jade is not setup, and the user declined to do it on Jade screen
                    Err(Error::NotInitialized)
                }
            }
            IsAuthResult::AuthResult(mut result) => {
                let client = reqwest::Client::new();

                loop {
                    let url = result.url(false).ok_or(Error::NoUsableUrl)?;
                    let str = serde_json::to_string(result.data())?;
                    let value: serde_json::Value = serde_json::from_str(&str)?;
                    log::debug!("POSTING to {url} data: {value}",);
                    let resp = client.post(url).json(&value).send().await?;
                    let status_code = resp.status().as_u16();
                    if status_code != 200 {
                        return Err(Error::HttpStatus(url.to_string(), status_code));
                    }
                    let bytes = &resp.bytes().await?;
                    let value: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
                    log::debug!("RECEIVED from {url} data: {value:?}");

                    let params: serde_cbor::Value = json_to_cbor(&value)?;

                    let method = result.on_reply().to_string();
                    let value = self.generic(method, params).await?;
                    if let serde_cbor::Value::Bool(val) = &value {
                        if *val {
                            return Ok(());
                        } else {
                            return Err(Error::HandshakeFailed);
                        }
                    }
                    result = serde_cbor::from_slice(&serde_cbor::to_vec(&value)?)?;
                }
            }
        }
    }

    pub async fn get_receive_address_single(
        &self,
        variant: Variant,
        path: Vec<u32>,
    ) -> Result<String> {
        let params = GetReceiveAddressParams {
            network: self.network,
            address: SingleOrMulti::Single { variant, path },
        };
        self.get_receive_address(params).await
    }

    pub async fn get_receive_address_multi(
        &self,
        name: &str,
        paths: Vec<Vec<u32>>,
    ) -> Result<String> {
        let params = GetReceiveAddressParams {
            network: self.network,
            address: SingleOrMulti::Multi {
                multisig_name: name.to_string(),
                paths,
            },
        };
        self.get_receive_address(params).await
    }

    pub fn network(&self) -> Network {
        self.network
    }

    // Should be implemented via the Signer trait, but here we are async...
    pub async fn slip77_master_blinding_key(&self) -> Result<slip77::MasterBlindingKey> {
        let params = GetMasterBlindingKeyParams {
            only_if_silent: false,
        };
        let bytes = self.get_master_blinding_key(params).await?;
        let array: [u8; 32] = bytes
            .to_vec()
            .try_into()
            .map_err(|_| Error::Slip77MasterBlindingKeyInvalidSize)?;
        Ok(slip77::MasterBlindingKey::from(array))
    }

    pub(crate) async fn send<T>(&self, request: Request) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        if let Some(network) = request.network() {
            self.check_network(network)?;
        }
        let buf = request.serialize()?;

        self.stream.write(&buf).await?;

        let mut rx = [0u8; 4096];

        let mut total = 0;
        loop {
            match self.stream.read(&mut rx[total..]).await {
                Ok(len) => {
                    total += len;
                    let reader = &rx[..total];

                    if let Some(value) = try_parse_response(reader) {
                        return value;
                    }
                }
                Err(Error::IoError(e)) if e.kind() == ErrorKind::Interrupted => (),

                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}
