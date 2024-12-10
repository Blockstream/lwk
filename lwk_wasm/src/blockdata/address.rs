use crate::{Error, Script};
use lwk_wollet::elements::{self, AddressParams};
use wasm_bindgen::prelude::*;

/// Wrapper of [`elements::Address`]
#[wasm_bindgen]
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
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Address, Error> {
        let inner: elements::Address = s.parse()?;
        Ok(inner.into())
    }

    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey().into()
    }

    #[wasm_bindgen(js_name = isBlinded)]
    pub fn is_blinded(&self) -> bool {
        self.inner.is_blinded()
    }

    #[wasm_bindgen(js_name = isMainnet)]
    pub fn is_mainnet(&self) -> bool {
        self.inner.params == &AddressParams::LIQUID
    }

    #[wasm_bindgen(js_name = toUnconfidential)]
    pub fn to_unconfidential(&self) -> Address {
        self.inner.to_unconfidential().into()
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
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

/// Wrapper of [`lwk_wollet::AddressResult`]
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
    pub fn address(&self) -> Address {
        self.inner.address().into()
    }

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

        assert_eq!(address.qr_code_uri(None).unwrap(), "data:image/bmp;base64,Qk2GAQAAAAAAAD4AAAAoAAAAKQAAACkAAAABAAEAAAAAAEgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+rhsdLwAAAIIBYidDgAAAuitpGseAAAC6FxQO0AAAALqGM/j4gAAAghPrII2AAAD+hUGKrAAAAACdlV+PgAAAw5WVyv2AAAAUfcT/9gAAAD62KlcnAAAAqV5aRQcAAADLW8XukAAAAAmtIQ39AAAA0sDx+G0AAAA4q8MaVAAAAOJCysWLgAAAQFCHbgKAAAB2Pxvq2oAAAMT876hGgAAA2ueBU1MAAAC4AQzPZYAAAI6ot+xlgAAA0fxBqruAAADX4QbxQAAAAKgn3wI9AAAA9mvTjNQAAADhUNCr54AAANcOWlNNAAAAxKq3TqUAAACnH0+yiIAAAFi4oJQIAAAAi8J7NXyAAAAAvg4kAAAAAP6qqqq/gAAAgtYuIaCAAAC6/AzSLoAAALrgXA4ugAAAuiJqsa6AAACCIz/toIAAAP7clm2/gAAA");
    }
}
