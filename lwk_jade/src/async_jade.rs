use std::{
    collections::HashMap,
    io::ErrorKind,
    time::{SystemTime, UNIX_EPOCH},
};

use elements::bitcoin::bip32::{Fingerprint, Xpub};
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    get_receive_address::GetReceiveAddressParams,
    protocol::{
        AuthResult, AuthUserParams, DebugSetMnemonicParams, EntropyParams, EpochParams,
        GetMasterBlindingKeyParams, GetSignatureParams, GetXpubParams, HandshakeCompleteParams,
        HandshakeData, HandshakeInitParams, IsAuthResult, Params, Request, Response,
        SignMessageParams, UpdatePinserverParams, VersionInfoResult,
    },
    register_multisig::{
        GetRegisteredMultisigParams, RegisterMultisigParams, RegisteredMultisig,
        RegisteredMultisigDetails,
    },
    sign_liquid_tx::{SignLiquidTxParams, TxInputParams},
};
use crate::{Error, Network, Result};

#[derive(Debug)]
pub struct AsyncJade<S: AsyncReadExt + AsyncWriteExt + Unpin> {
    /// Jade working via emulator(tcp), physical(serial/bluetooth)
    stream: S,

    /// The network
    network: Network,

    /// Cached master xpub
    master_xpub: Option<Xpub>,
}

#[cfg(feature = "tcp")]
impl AsyncJade<tokio::net::TcpStream> {
    pub async fn new_tcp(stream: tokio::net::TcpStream, network: Network) -> Self {
        Self {
            stream,
            network,
            master_xpub: None,
        }
    }
}

#[cfg(feature = "serial")]
impl AsyncJade<tokio_serial::SerialStream> {
    pub async fn new_serial(stream: tokio_serial::SerialStream, network: Network) -> Self {
        Self {
            stream,
            network,
            master_xpub: None,
        }
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Unpin> AsyncJade<S> {
    fn check_network(&self, passed: Network) -> Result<()> {
        let init = self.network;
        if passed != init {
            Err(Error::MismatchingXpub { init, passed })
        } else {
            Ok(())
        }
    }

    pub async fn ping(&mut self) -> Result<u8> {
        self.send_request("ping", None).await
    }

    pub async fn logout(&mut self) -> Result<bool> {
        self.send_request("logout", None).await
    }

    pub async fn version_info(&mut self) -> Result<VersionInfoResult> {
        self.send_request("get_version_info", None).await
    }

    pub async fn set_epoch(&mut self, epoch: u64) -> Result<bool> {
        let params = Params::Epoch(EpochParams { epoch });
        self.send_request("set_epoch", Some(params)).await
    }

    pub async fn add_entropy(&mut self, entropy: &[u8]) -> Result<bool> {
        let params = Params::Entropy(EntropyParams {
            entropy: entropy.to_vec(),
        });
        self.send_request("add_entropy", Some(params)).await
    }

    pub async fn auth_user(&mut self) -> Result<IsAuthResult<String>> {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(Error::SystemTimeError)?
            .as_secs();
        let params = Params::AuthUser(AuthUserParams {
            network: self.network,
            epoch,
        });
        self.send_request("auth_user", Some(params)).await
    }

    pub async fn handshake_init(
        &mut self,
        params: HandshakeInitParams,
    ) -> Result<AuthResult<HandshakeData>> {
        let params = Params::HandshakeInit(params);
        self.send_request("handshake_init", Some(params)).await
    }

    pub async fn update_pinserver(&mut self, params: UpdatePinserverParams) -> Result<bool> {
        let params = Params::UpdatePinServer(params);
        self.send_request("update_pinserver", Some(params)).await
    }

    pub async fn handshake_complete(&mut self, params: HandshakeCompleteParams) -> Result<bool> {
        let params = Params::HandshakeComplete(params);
        self.send_request("handshake_complete", Some(params)).await
    }

    async fn inner_get_xpub(&mut self, params: GetXpubParams) -> Result<Xpub> {
        self.check_network(params.network)?;
        let params = Params::GetXpub(params);
        self.send_request("get_xpub", Some(params)).await
    }

    pub async fn get_xpub(&mut self, params: GetXpubParams) -> Result<Xpub> {
        if params.path.is_empty() {
            self.get_master_xpub().await
        } else {
            self.inner_get_xpub(params).await
        }
    }

    pub async fn fingerprint(&mut self) -> Result<Fingerprint> {
        Ok(self.get_master_xpub().await?.fingerprint())
    }

    pub async fn get_master_xpub(&mut self) -> Result<Xpub> {
        if self.master_xpub.is_none() {
            let master_xpub = self
                .inner_get_xpub(GetXpubParams {
                    network: self.network,
                    path: vec![],
                })
                .await?;
            self.master_xpub = Some(master_xpub);
        }
        Ok(self.master_xpub.expect("ensure it is some before"))
    }

    pub async fn get_receive_address(&mut self, params: GetReceiveAddressParams) -> Result<String> {
        self.check_network(params.network)?;
        let params = Params::GetReceiveAddress(params);
        self.send_request("get_receive_address", Some(params)).await
    }

    pub async fn get_master_blinding_key(
        &mut self,
        params: GetMasterBlindingKeyParams,
    ) -> Result<ByteBuf> {
        let params = Params::GetMasterBlindingKey(params);
        self.send_request("get_master_blinding_key", Some(params))
            .await
    }

    pub async fn sign_message(&mut self, params: SignMessageParams) -> Result<ByteBuf> {
        let params = Params::SignMessage(params);
        self.send_request("sign_message", Some(params)).await
    }

    pub async fn get_signature_for_msg(&mut self, params: GetSignatureParams) -> Result<String> {
        let params = Params::GetSignature(params);
        self.send_request("get_signature", Some(params)).await
    }

    pub async fn get_signature_for_tx(&mut self, params: GetSignatureParams) -> Result<ByteBuf> {
        let params = Params::GetSignature(params);
        self.send_request("get_signature", Some(params)).await
    }

    pub async fn sign_liquid_tx(&mut self, params: SignLiquidTxParams) -> Result<bool> {
        self.check_network(params.network)?;
        let params = Params::SignLiquidTx(params);
        self.send_request("sign_liquid_tx", Some(params)).await
    }

    pub async fn tx_input(&mut self, params: TxInputParams) -> Result<ByteBuf> {
        let params = Params::TxInput(params);
        self.send_request("tx_input", Some(params)).await
    }

    pub async fn debug_set_mnemonic(&mut self, params: DebugSetMnemonicParams) -> Result<bool> {
        let params = Params::DebugSetMnemonic(params);
        self.send_request("debug_set_mnemonic", Some(params)).await
    }

    pub async fn register_multisig(&mut self, params: RegisterMultisigParams) -> Result<bool> {
        self.check_network(params.network)?;
        let params = Params::RegisterMultisig(params);
        self.send_request("register_multisig", Some(params)).await
    }

    pub async fn get_registered_multisigs(
        &mut self,
    ) -> Result<HashMap<String, RegisteredMultisig>> {
        self.send_request("get_registered_multisigs", None).await
    }

    pub async fn get_registered_multisig(
        &mut self,
        params: GetRegisteredMultisigParams,
    ) -> Result<RegisteredMultisigDetails> {
        let params = Params::GetRegisteredMultisig(params);
        self.send_request("get_registered_multisig", Some(params))
            .await
    }

    async fn send_request<T>(&mut self, method: &str, params: Option<Params>) -> Result<T>
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
        serde_cbor::to_writer(&mut buf, &req)?;
        tracing::debug!(
            "\n--->\t{:#?}\n\t({} bytes) {}",
            &req,
            buf.len(),
            &hex::encode(&buf),
        );

        self.stream.write_all(&buf).await?;

        let mut rx = [0u8; 4096];

        let mut total = 0;
        loop {
            match self.stream.read(&mut rx[total..]).await {
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
