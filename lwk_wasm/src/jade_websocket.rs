use std::{collections::HashMap, str::FromStr, sync::OnceLock};

use crate::jade::Singlesig;
use crate::{signer::FakeSigner, Bip, Error, Network, Pset, WolletDescriptor, Xpub};
use lwk_common::{DescriptorBlindingKey, Signer};
use lwk_jade::asyncr::Stream;
use lwk_jade::{asyncr, protocol::GetXpubParams};
use lwk_jade::{
    derivation_path_to_vec,
    get_receive_address::{GetReceiveAddressParams, SingleOrMulti},
    register_multisig::{JadeDescriptor, RegisterMultisigParams, RegisteredMultisigDetails},
};
use lwk_wollet::elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use lwk_wollet::{bitcoin::bip32::DerivationPath, elements::pset::PartiallySignedTransaction};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};

/// WebSocket-based implementation of the Stream trait for Jade communication
pub struct WebSocketSerial {
    websocket: WebSocket,
    receive_buffer: Arc<Mutex<VecDeque<u8>>>,
}

impl WebSocketSerial {
    pub async fn new(url: &str) -> Result<Self, Error> {
        let websocket = WebSocket::new(url).map_err(Error::JsVal)?;
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let receive_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let buffer_clone = receive_buffer.clone();

        // Set up message handler
        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(array_buffer) = e.data().dyn_into::<web_sys::js_sys::ArrayBuffer>() {
                let uint8_array = web_sys::js_sys::Uint8Array::new(&array_buffer);
                let data = uint8_array.to_vec();
                if let Ok(mut buffer) = buffer_clone.lock() {
                    buffer.extend(data);
                }
            }
        }) as Box<dyn FnMut(_)>);

        websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Wait for connection to open
        let connection_promise = web_sys::js_sys::Promise::new(&mut |resolve, _reject| {
            let onopen_callback = Closure::wrap(Box::new(move |_event: Event| {
                resolve.call0(&JsValue::NULL).unwrap();
            }) as Box<dyn FnMut(Event)>);

            websocket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();
        });

        wasm_bindgen_futures::JsFuture::from(connection_promise)
            .await
            .map_err(Error::JsVal)?;

        Ok(Self {
            websocket,
            receive_buffer,
        })
    }
}

impl Stream for WebSocketSerial {
    async fn read(&self, buf: &mut [u8]) -> Result<usize, lwk_jade::Error> {
        // Try to read from buffer first
        loop {
            {
                let mut buffer = self
                    .receive_buffer
                    .lock()
                    .map_err(|e| lwk_jade::Error::Generic(format!("Mutex error: {}", e)))?;

                if !buffer.is_empty() {
                    let read_len = std::cmp::min(buf.len(), buffer.len());
                    for i in 0..read_len {
                        buf[i] = buffer.pop_front().unwrap();
                    }
                    return Ok(read_len);
                }
            }

            // Wait a bit for more data
            let promise = web_sys::js_sys::Promise::new(&mut |resolve, _reject| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10)
                    .unwrap();
            });

            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(|e| lwk_jade::Error::Generic(format!("Timeout error: {:?}", e)))?;
        }
    }

    async fn write(&self, buf: &[u8]) -> Result<(), lwk_jade::Error> {
        let uint8_array = web_sys::js_sys::Uint8Array::new_with_length(buf.len() as u32);
        uint8_array.copy_from(buf);

        self.websocket
            .send_with_array_buffer(&uint8_array.buffer())
            .map_err(|e| lwk_jade::Error::Generic(format!("WebSocket send error: {:?}", e)))?;

        Ok(())
    }
}

/// WebSocket-based Wrapper of [`asyncr::Jade`]
#[wasm_bindgen]
pub struct JadeWebSocket {
    inner: asyncr::Jade<WebSocketSerial>,
    _websocket: WebSocket,
    fake_signer: OnceLock<FakeSigner>,
}

// NOTE: Every exposed method (`pub async`) needs to try to unlock the jade as first step.
#[wasm_bindgen]
impl JadeWebSocket {
    /// Creates a Jade from WebSocket for the given network
    ///
    /// The url should point to your WebSocket bridge that connects to the Docker Jade emulator
    #[wasm_bindgen(constructor)]
    pub async fn from_websocket(network: &Network, url: &str) -> Result<JadeWebSocket, Error> {
        let websocket_serial = WebSocketSerial::new(url).await?;
        let websocket = websocket_serial.websocket.clone();

        let inner = asyncr::Jade::new(websocket_serial, network.clone().into());
        Ok(JadeWebSocket {
            inner,
            _websocket: websocket,
            fake_signer: OnceLock::new(),
        })
    }

    #[wasm_bindgen(js_name = getVersion)]
    pub async fn get_version(&self) -> Result<JsValue, Error> {
        let version = self.inner.version_info().await?;
        Ok(serde_wasm_bindgen::to_value(&version)?)
    }

    #[wasm_bindgen(js_name = getMasterXpub)]
    pub async fn get_master_xpub(&self) -> Result<Xpub, Error> {
        self.inner.unlock().await?;
        let xpub = self.inner.get_master_xpub().await?;
        Ok(xpub.into())
    }

    /// Return a single sig address with the given `variant` and `path` derivation
    #[wasm_bindgen(js_name = getReceiveAddressSingle)]
    pub async fn get_receive_address_single(
        &self,
        variant: Singlesig,
        path: Vec<u32>,
    ) -> Result<String, Error> {
        self.inner.unlock().await?;
        let network = self.inner.network();
        let xpub = self
            .inner
            .get_receive_address(GetReceiveAddressParams {
                network,
                address: SingleOrMulti::Single {
                    variant: variant.into(),
                    path,
                },
            })
            .await?;
        Ok(xpub.to_string())
    }

    /// Return a multisig address of a registered `multisig_name` wallet
    ///
    /// This method accept `path` and `path_n` in place of a single `Vec<Vec<u32>>` because the
    /// latter is not supported by wasm_bindgen (and neither `(u32, Vec<u32>)`). `path` and `path_n`
    /// are converted internally to a `Vec<Vec<u32>>` with the caveat all the paths are the same,
    /// which is almost always the case.
    #[wasm_bindgen(js_name = getReceiveAddressMulti)]
    pub async fn get_receive_address_multi(
        &self,
        multisig_name: &str,
        path: Vec<u32>,
    ) -> Result<String, Error> {
        self.inner.unlock().await?;
        let network = self.inner.network();
        let multi_details = self.get_registered_multisig(multisig_name).await?;
        let path_n = multi_details.descriptor.signers.len();
        let mut paths = vec![];
        for _ in 0..path_n {
            paths.push(path.clone());
        }
        let xpub = self
            .inner
            .get_receive_address(GetReceiveAddressParams {
                network,
                address: SingleOrMulti::Multi {
                    multisig_name: multisig_name.to_string(),
                    paths,
                },
            })
            .await?;
        Ok(xpub.to_string())
    }

    /// Sign and consume the given PSET, returning the signed one
    pub async fn sign(&self, pset: Pset) -> Result<Pset, Error> {
        self.inner.unlock().await?;
        let mut pset: PartiallySignedTransaction = pset.into();
        self.inner.sign(&mut pset).await?;
        Ok(pset.into())
    }

    pub async fn wpkh(&self) -> Result<WolletDescriptor, Error> {
        self.inner.unlock().await?;
        self.desc(lwk_common::Singlesig::Wpkh).await
    }

    #[wasm_bindgen(js_name = shWpkh)]
    pub async fn sh_wpkh(&self) -> Result<WolletDescriptor, Error> {
        self.inner.unlock().await?;
        self.desc(lwk_common::Singlesig::ShWpkh).await
    }

    pub async fn multi(&self, name: &str) -> Result<WolletDescriptor, Error> {
        self.inner.unlock().await?;
        let r = self.get_registered_multisig(name).await?;
        let desc: ConfidentialDescriptor<DescriptorPublicKey> = (&r.descriptor)
            .try_into()
            .map_err(|s| Error::Generic(format!("{:?}", s)))?;

        Ok(lwk_wollet::WolletDescriptor::try_from(desc)
            .map_err(|s| Error::Generic(format!("{:?}", s)))?
            .into())
    }

    #[wasm_bindgen(js_name = getRegisteredMultisigs)]
    pub async fn get_registered_multisigs(&self) -> Result<JsValue, Error> {
        self.inner.unlock().await?;
        let wallets = self.inner.get_registered_multisigs().await?;
        let wallets_str: Vec<_> = wallets.keys().collect();
        Ok(serde_wasm_bindgen::to_value(&wallets_str)?)
    }

    #[wasm_bindgen(js_name = keyoriginXpub)]
    pub async fn keyorigin_xpub(&self, bip: &Bip) -> Result<String, Error> {
        let signer = self.get_or_create_fake_signer().await?;
        let is_mainnet = self.inner.network().is_mainnet();

        Ok(signer
            .keyorigin_xpub(bip.into(), is_mainnet)
            .map_err(Error::Generic)?)
    }

    #[wasm_bindgen(js_name = registerDescriptor)]
    pub async fn register_descriptor(
        &self,
        name: &str,
        desc: &WolletDescriptor,
    ) -> Result<bool, Error> {
        self.inner.unlock().await?;
        let descriptor: JadeDescriptor = desc.as_ref().as_ref().try_into().unwrap();
        let network = self.inner.network();
        let result = self
            .inner
            .register_multisig(RegisterMultisigParams {
                descriptor,
                multisig_name: name.to_string(),
                network,
            })
            .await?;
        Ok(result)
    }
}

// Inner methods don't try to unlock, they are supposed to operate on an already unlocked Jade,
impl JadeWebSocket {
    async fn desc(&self, script_variant: lwk_common::Singlesig) -> Result<WolletDescriptor, Error> {
        let signer = self.get_or_create_fake_signer().await?;

        let desc_str =
            lwk_common::singlesig_desc(signer, script_variant, DescriptorBlindingKey::Slip77)
                .map_err(Error::Generic)?;
        WolletDescriptor::new(&desc_str)
    }

    async fn get_registered_multisig(
        &self,
        name: &str,
    ) -> Result<RegisteredMultisigDetails, Error> {
        // TODO should call a cached methods to minimize roundtrip on websocket
        let param = lwk_jade::register_multisig::GetRegisteredMultisigParams {
            multisig_name: name.to_string(),
        };
        let r = self.inner.get_registered_multisig(param).await?;
        Ok(r)
    }

    async fn get_or_create_fake_signer(&self) -> Result<&FakeSigner, Error> {
        if let Some(signer) = self.fake_signer.get() {
            return Ok(signer);
        }

        self.inner.unlock().await?;
        let network = self.inner.network();
        let mut paths = HashMap::new();

        for purpose in [49, 84, 87] {
            for coin_type in [1, 1776] {
                let derivation_path_str = format!("m/{purpose}h/{coin_type}h/0h");
                let derivation_path = DerivationPath::from_str(&derivation_path_str)?;
                let path = derivation_path_to_vec(&derivation_path);
                let params = GetXpubParams { network, path };
                let xpub = self.inner.get_cached_xpub(params).await?;
                paths.insert(derivation_path, xpub);
            }
        }
        let xpub = self.inner.get_master_xpub().await?;
        paths.insert(DerivationPath::master(), xpub);
        let slip77 = self.inner.slip77_master_blinding_key().await?;

        let signer = FakeSigner { paths, slip77 };
        self.fake_signer.set(signer).unwrap();
        Ok(self.fake_signer.get().unwrap())
    }
}
