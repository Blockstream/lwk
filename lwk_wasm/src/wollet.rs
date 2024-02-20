use crate::{Error, Network, Update, WolletDescriptor};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Wollet {
    inner: lwk_wollet::Wollet,
}

impl AsRef<lwk_wollet::Wollet> for Wollet {
    fn as_ref(&self) -> &lwk_wollet::Wollet {
        &self.inner
    }
}

impl AsMut<lwk_wollet::Wollet> for Wollet {
    fn as_mut(&mut self) -> &mut lwk_wollet::Wollet {
        &mut self.inner
    }
}

#[wasm_bindgen]
impl Wollet {
    /// Create a new  wallet
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Result<Wollet, Error> {
        let inner = lwk_wollet::Wollet::without_persist(network.into(), descriptor.into())?;
        Ok(Self { inner })
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<String, Error> {
        // TODO return AddressResult
        let address = self.inner.address(index)?;
        Ok(address.address().to_string())
    }

    pub fn apply_update(&mut self, update: Update) -> Result<(), Error> {
        Ok(self.inner.apply_update(update.into())?)
    }

    pub fn balance(&self) -> Result<JsValue, Error> {
        let balance = self.inner.balance()?;
        Ok(serde_wasm_bindgen::to_value(&balance)?)
    }
}

mod tests {

    use wasm_bindgen_test::*;

    use crate::{Network, Wollet, WolletDescriptor};
    wasm_bindgen_test_configure!(run_in_browser);

    const DESCRIPTOR: &str = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";

    #[wasm_bindgen_test]
    fn test_wollet_address() {
        let descriptor = WolletDescriptor::new(DESCRIPTOR).unwrap();
        let network = Network::mainnet();
        let wollet = Wollet::new(network, descriptor).unwrap();
        assert_eq!(
            wollet.address(Some(0)).unwrap(),
            "VJLAQiChRTcVDXEBKrRnSBnGccJLxNg45zW8cuDwkhbxb8NVFkb4U2QMWAzot4idqhLMWjtZ7SXA4nrA"
        );
    }

    #[wasm_bindgen_test]
    async fn test_balance() {
        let descriptor = WolletDescriptor::new(DESCRIPTOR).unwrap();
        let network = Network::mainnet();
        let mut client = network.default_esplora_client();

        let mut wollet = Wollet::new(network, descriptor).unwrap();
        let update = client.full_scan(&wollet).await.unwrap().unwrap();
        wollet.apply_update(update).unwrap();
        let balance = wollet.balance().unwrap();
        println!("{:?}", balance);
    }
}
