use crate::{
    serial::{get_jade_serial, WebSerial},
    Error, Network,
};
use lwk_jade::asyncr;
use lwk_jade::get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant};
use wasm_bindgen::prelude::*;

/// Wrapper of [`asyncr::Jade`]
#[wasm_bindgen]
pub struct Jade {
    inner: asyncr::Jade<WebSerial>,
    _port: web_sys::SerialPort,
}

#[wasm_bindgen]
impl Jade {
    /// Creates a Jade from Web Serial for the given network
    ///
    /// When filter is true, it will filter available serial with Blockstream released chips, use
    /// false if you don't see your DYI jade
    #[wasm_bindgen(constructor)]
    pub async fn from_serial(network: Network, filter: bool) -> Result<Jade, Error> {
        let port = get_jade_serial(filter).await?;
        let web_serial = WebSerial::new(&port)?;

        let inner = asyncr::Jade::new(web_serial, network.into());
        Ok(Jade { inner, _port: port })
    }

    #[wasm_bindgen(js_name = getVersion)]
    pub async fn get_version(&self) -> Result<JsValue, Error> {
        let version = self.inner.version_info().await?;
        Ok(serde_wasm_bindgen::to_value(&version)?)
    }

    #[wasm_bindgen(js_name = getMasterXpub)]
    pub async fn get_master_xpub(&self) -> Result<String, Error> {
        self.inner.unlock().await?;
        let xpub = self.inner.get_master_xpub().await?;
        Ok(xpub.to_string())
    }

    /// Return a single sig address with the given `variant` and `path` derivation
    #[wasm_bindgen(js_name = getReceiveAddressSingle)]
    pub async fn get_receive_address_single(
        &self,
        variant: SingleVariant,
        path: Vec<u32>,
    ) -> Result<String, Error> {
        let network = self.inner.network();
        self.inner.unlock().await?;
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
        multisig_name: String,
        path: Vec<u32>,
        path_n: u32,
    ) -> Result<String, Error> {
        let network = self.inner.network();
        self.inner.unlock().await?;
        let mut paths = vec![];
        for _ in 0..path_n {
            paths.push(path.clone());
        }
        let xpub = self
            .inner
            .get_receive_address(GetReceiveAddressParams {
                network,
                address: SingleOrMulti::Multi {
                    multisig_name,
                    paths,
                },
            })
            .await?;
        Ok(xpub.to_string())
    }
}

#[wasm_bindgen]
pub enum SingleVariant {
    /// Witness Public Key Hash or native segwit
    Wpkh,

    /// Script Hash Witness Public Key Hash or wrapped segwit
    ShWpkh,
}

impl From<SingleVariant> for Variant {
    fn from(v: SingleVariant) -> Self {
        match v {
            SingleVariant::Wpkh => Variant::Wpkh,
            SingleVariant::ShWpkh => Variant::ShWpkh,
        }
    }
}

impl From<Variant> for SingleVariant {
    fn from(v: Variant) -> Self {
        match v {
            Variant::Wpkh => SingleVariant::Wpkh,
            Variant::ShWpkh => SingleVariant::ShWpkh,
        }
    }
}
