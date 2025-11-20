use crate::{Error, Network, Script};
use lwk_wollet::elements::{self, AddressParams};
use wasm_bindgen::prelude::*;

/// An Elements (Liquid) address
#[wasm_bindgen]
#[derive(Debug)]
pub struct Address {
    inner: elements::Address,
}

impl From<elements::Address> for Address {
    fn from(inner: elements::Address) -> Self {
        Self { inner }
    }
}

impl From<&elements::Address> for Address {
    fn from(inner: &elements::Address) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

impl From<Address> for elements::Address {
    fn from(addr: Address) -> Self {
        addr.inner
    }
}

impl AsRef<elements::Address> for Address {
    fn as_ref(&self) -> &elements::Address {
        &self.inner
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Address {
    /// Creates an `Address`
    ///
    /// If you know the network, you can use `parse()` to validate that the network is consistent.
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Address, Error> {
        let inner: elements::Address = s.parse()?;
        Ok(inner.into())
    }

    /// Parses an `Address` ensuring is for the right network
    pub fn parse(s: &str, network: &Network) -> Result<Address, Error> {
        let common_addr = lwk_common::Address::parse(s, (*network).into())?;
        let inner: elements::Address = common_addr.into();
        Ok(inner.into())
    }

    /// Return the script pubkey of the address.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey().into()
    }

    /// Return true if the address is blinded, in other words, if it has a blinding key.
    #[wasm_bindgen(js_name = isBlinded)]
    pub fn is_blinded(&self) -> bool {
        self.inner.is_blinded()
    }

    /// Return true if the address is for mainnet.
    #[wasm_bindgen(js_name = isMainnet)]
    pub fn is_mainnet(&self) -> bool {
        self.inner.params == &AddressParams::LIQUID
    }

    /// Return the unconfidential address, in other words, the address without the blinding key.
    #[wasm_bindgen(js_name = toUnconfidential)]
    pub fn to_unconfidential(&self) -> Address {
        self.inner.to_unconfidential().into()
    }

    /// Return the string representation of the address.
    /// This representation can be used to recreate the address via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Returns a string encoding an image in a uri
    ///
    /// The string can be open in the browser or be used as `src` field in `img` in HTML
    ///
    /// For max efficiency we suggest to pass `None` to `pixel_per_module`, get a very small image
    /// and use styling to scale up the image in the browser. eg
    /// `style="image-rendering: pixelated; border: 20px solid white;"`
    #[wasm_bindgen(js_name = QRCodeUri)]
    pub fn qr_code_uri(&self, pixel_per_module: Option<u8>) -> Result<String, Error> {
        Ok(lwk_common::address_to_uri_qr(
            &self.inner,
            pixel_per_module,
        )?)
    }

    /// Returns a string of the QR code printable in a terminal environment
    #[wasm_bindgen(js_name = QRCodeText)]
    pub fn qr_code_text(&self) -> Result<String, Error> {
        Ok(lwk_common::address_to_text_qr(&self.inner)?)
    }
}

/// Value returned from asking an address to the wallet.
/// Containing the confidential address and its
/// derivation index (the last element in the derivation path)
#[wasm_bindgen]
pub struct AddressResult {
    inner: lwk_wollet::AddressResult,
}

impl From<lwk_wollet::AddressResult> for AddressResult {
    fn from(inner: lwk_wollet::AddressResult) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl AddressResult {
    /// Return the address.
    pub fn address(&self) -> Address {
        self.inner.address().into()
    }

    /// Return the derivation index of the address.
    pub fn index(&self) -> u32 {
        self.inner.index()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use super::Address;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn address() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";

        let address = Address::new(address_str).unwrap();
        assert_eq!(address.to_string(), address_str);

        assert!(address.is_blinded());

        assert_eq!(
            address.to_unconfidential().to_string(),
            "tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8"
        );

        assert_eq!(
            address.script_pubkey().to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );

        assert_eq!(address.qr_code_uri(None).unwrap(), "data:image/bmp;base64,Qk2GAQAAAAAAAD4AAAAoAAAAKQAAACkAAAABAAEAAAAAAEgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+rtsVLwAAAIJnIiVDgAAAuk9JGoeAAAC6U1QO0AAAALqGM/j4gAAAghPrII2AAAD+hUGKrAAAAACdlV+PgAAA25WVyv2AAAAcfcT/9gAAAD62KlcnAAAAqV5aRQcAAADLSsXvkAAAAAmloRT9AAAA0tHx8G0AAAA4qEMaVAAAAOIoSs2LgAAAQHSHbAKAAAB2Hxvu2oAAAMS476hGgAAA2ueBU1MAAACYAQzPZYAAAL6ot+xlgAAAoPxBqruAAACHYQbxQAAAAOgn3wI9AAAAdmvTjNQAAABhUNCr54AAANceWkNNAAAAxKM3VqUAAACnB0+6iIAAAFiioJQIAAAAi6B7NXyAAAAA3g4mAAAAAP6qqqq/gAAAgrAuJ6CAAAC6vAzSLoAAALrgXA4ugAAAuiJqsa6AAACCIz/toIAAAP7clm2/gAAA");

        let address_network_check = Address::parse(
            address_str,
            &lwk_wollet::ElementsNetwork::LiquidTestnet.into(),
        )
        .unwrap();
        assert_eq!(address_network_check.to_string(), address_str);

        let address_network_check_fail =
            Address::parse(address_str, &lwk_wollet::ElementsNetwork::Liquid.into()).unwrap_err();
        assert_eq!(
            address_network_check_fail.to_string(),
            "Expected a mainnet address but got a testnet one"
        );
    }
}
