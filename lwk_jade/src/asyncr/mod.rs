use std::{collections::HashMap, io::ErrorKind};

use crate::get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant};
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
use tokio::sync::Mutex;
use web_sys::js_sys::Uint8Array;

#[derive(Debug)]
pub struct Jade {
    reader: web_sys::ReadableStreamDefaultReader,

    writer: web_sys::WritableStreamDefaultWriter,

    /// The network
    network: Network,

    /// Cached master xpub
    cached_xpubs: Mutex<HashMap<DerivationPath, Xpub>>,
}

impl Jade {
    pub fn new(serial: &web_sys::SerialPort, network: Network) -> Jade {
        Jade {
            reader: web_sys::ReadableStreamDefaultReader::new(&serial.readable()).unwrap(),
            writer: serial.writable().get_writer().unwrap(),
            network,
            cached_xpubs: Mutex::new(HashMap::new()),
        }
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

    pub async fn auth_user(&self, params: AuthUserParams) -> Result<IsAuthResult<String>> {
        self.send(Request::AuthUser(params)).await
    }

    pub async fn handshake_init(
        &self,
        params: HandshakeInitParams,
    ) -> Result<AuthResult<HandshakeData>> {
        self.send(Request::HandshakeInit(params)).await
    }

    pub async fn update_pinserver(&self, params: UpdatePinserverParams) -> Result<bool> {
        self.send(Request::UpdatePinserver(params)).await
    }

    pub async fn handshake_complete(&self, params: HandshakeCompleteParams) -> Result<bool> {
        self.send(Request::HandshakeComplete(params)).await
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

    pub async fn sign_message(&self, params: SignMessageParams) -> Result<ByteBuf> {
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
        self.send(Request::RegisterMultisig(params)).await
    }

    pub async fn get_registered_multisigs(&self) -> Result<HashMap<String, RegisteredMultisig>> {
        self.send(Request::GetRegisteredMultisigs).await
    }

    pub async fn get_registered_multisig(
        &self,
        params: GetRegisteredMultisigParams,
    ) -> Result<RegisteredMultisigDetails> {
        self.send(Request::GetRegisteredMultisig(params)).await
    }

    pub async fn get_cached_xpub(&self, params: GetXpubParams) -> Result<Xpub> {
        if let Some(xpub) = self
            .cached_xpubs
            .lock()
            .await
            .get(&vec_to_derivation_path(&params.path))
        {
            Ok(*xpub)
        } else {
            self.get_xpub(params).await
        }
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
        match self.auth_user(AuthUserParams::new(self.network)?).await? {
            IsAuthResult::AlreadyAuth(result) => {
                if result {
                    Ok(())
                } else {
                    // Jade is not setup, and the user declined to do it on Jade screen
                    Err(Error::NotInitialized)
                }
            }
            IsAuthResult::AuthResult(result) => {
                let client = reqwest::Client::new();
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let resp = client.post(url).send().await?;
                let status_code = resp.status().as_u16();
                if status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), status_code));
                }

                let params: HandshakeInitParams =
                    serde_json::from_slice(resp.bytes().await?.as_ref())?;
                let result = self.handshake_init(params).await?;
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let data = serde_json::to_vec(result.data())?;
                let resp = client.post(url).body(data).send().await?;
                let status_code = resp.status().as_u16();
                if status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), status_code));
                }
                let params: HandshakeCompleteParams =
                    serde_json::from_slice(resp.bytes().await?.as_ref())?;

                let result = self.handshake_complete(params).await?;

                if !result {
                    return Err(Error::HandshakeFailed);
                }

                Ok(())
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

    pub async fn network(&self) -> Network {
        self.network
    }

    pub(crate) async fn send<T>(&self, request: Request) -> Result<T>
    where
        T: std::fmt::Debug + DeserializeOwned,
    {
        web_sys::console::log_1(&"a".into());
        if let Some(network) = request.network() {
            self.check_network(network)?;
        }
        web_sys::console::log_1(&"a1".into());

        let buf = request.serialize()?;

        let arr = Uint8Array::new_with_length(buf.len() as u32);
        arr.copy_from(&buf);
        web_sys::console::log_1(&"b".into());
        web_sys::console::log_1(&"b1".into());

        let promise = self.writer.write_with_chunk(&arr);
        web_sys::console::log_1(&"c".into());

        wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
        web_sys::console::log_1(&"d".into());

        let mut rx = vec![];

        loop {
            let promise = self.reader.read();

            let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
            web_sys::console::log_1(&result);

            let value = web_sys::js_sys::Reflect::get(&result, &"value".into()).unwrap();

            let data = Uint8Array::new(&value);
            rx.extend(&data.to_vec());

            web_sys::console::log_1(&"x".into());

            if let Some(value) = try_parse_response(&rx) {
                web_sys::console::log_1(&"i".into());

                return value;
            }
            web_sys::console::log_1(&"l".into());
        }

        // match stream.read(&mut rx[total..]).await {
        //     Ok(len) => {
        //         total += len;
        //         let reader = &rx[..total];

        //         if let Some(value) = try_parse_response(reader) {
        //             return value;
        //         }
        //     }
        //     Err(e) => {
        //         if e.kind() != ErrorKind::Interrupted {
        //             dbg!(&e);
        //             return Err(Error::IoError(e));
        //         }
        //     }
        // }
    }
}
