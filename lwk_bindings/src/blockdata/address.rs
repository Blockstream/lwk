use crate::{LwkError, Script};
use std::{fmt::Display, sync::Arc};

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Address {
    inner: elements::Address,
}

impl From<elements::Address> for Address {
    fn from(inner: elements::Address) -> Self {
        Self { inner }
    }
}

impl AsRef<elements::Address> for Address {
    fn as_ref(&self) -> &elements::Address {
        &self.inner
    }
}

impl From<Address> for elements::Address {
    fn from(addr: Address) -> Self {
        addr.inner
    }
}

impl From<&Address> for elements::Address {
    fn from(addr: &Address) -> Self {
        addr.inner.clone()
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Address {
    /// Construct an Address object
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Address = s.parse()?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey().into())
    }

    pub fn is_blinded(&self) -> bool {
        self.inner.is_blinded()
    }

    pub fn to_unconfidential(&self) -> Arc<Self> {
        Arc::new(self.inner.to_unconfidential().into())
    }

    /// Returns a string encoding an image in a uri
    ///
    /// The string can be open in the browser or be used as `src` field in `img` in HTML
    ///
    /// For max efficiency we suggest to pass `None` to `pixel_per_module`, get a very small image
    /// and use styling to scale up the image in the browser. eg
    /// `style="image-rendering: pixelated; border: 20px solid white;"`
    pub fn qr_code_uri(&self, pixel_per_module: Option<u8>) -> Result<String, LwkError> {
        Ok(lwk_common::address_to_uri_qr(
            &self.inner,
            pixel_per_module,
        )?)
    }

    /// Returns a string of the QR code printable in a terminal environment
    pub fn qr_code_text(&self) -> Result<String, LwkError> {
        Ok(lwk_common::address_to_text_qr(&self.inner)?)
    }
}

#[cfg(test)]
mod tests {

    use super::Address;

    #[test]
    fn address() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";

        let address = Address::new(address_str).unwrap();
        assert_eq!(address.to_string(), address_str);

        assert_eq!(
            address.script_pubkey().to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );

        assert!(address.is_blinded());

        assert_eq!(
            address.to_unconfidential().to_string(),
            "tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8"
        );

        let expected = "
███████████████████████████████████████████████
███████████████████████████████████████████████
███ ▄▄▄▄▄ █▄▄▀▄▄▄▀▀▄█▀ ▀  ▀▀  █  █  █ ▄▄▄▄▄ ███
███ █   █ █▀▀ ███▄██ ▄▀ ▀▄█▄█▄▄▀▀▀▄▄█ █   █ ███
███ █▄▄▄█ █  ▄ ▄ ▀███▀█  ▀█▄▄▀▄██▄▀▀█ █▄▄▄█ ███
███▄▄▄▄▄▄▄█ █ ▀ ▀ █▄█▄█ ▀ █▄█ █▄▀▄█▄█▄▄▄▄▄▄▄███
███▄▀█▀ █▄▄ ▄▀▀▀█▄█▀▄ ▄▄█▄▄▀█▄ █ █▄█▄▄▄ ▄██▄███
███ ▀▄██ ▄▄▀█▀▄ ▄ ▄▀▄▀▀▄   ▄▀▄▄▀▀ █ █▀█▄▀█▀▄███
███  ▀▄█▄▄ █▀█▀▄▄▄█▀ █ ▄█▄█▀▄▀▄▀█  ▀ ▀█▄ ▀ ▀███
███ ▄ ▄▀▄▄██▄ █▄▀    █ ▀▀  ▄███▄▄▀█▄▄▀ ▀ █▀████
███  █ █▄▄    ▀▀▀█▄█▀███▄▄▀ ▄ ▄▀█▀▄▀▄▀▀▀█▀▀▀███
███ █▀▀ ▄▄█▄█▄█▄██▀▄█▄▄▀ ▄▄  ▄█  ▀▀█  ██ █  ███
███  █▄▄▀▄█   ▀▀ ▄▄ ▀▀█▀▀▀ ▀▄▀▄▀█▄▄█ █▄█▀ ▄▀███
████ ▄▄█▄▄██▀▄ ▄▄▄▄▀██▄▄▀  ▄  █ ▀ █▄▄█▄▄█ █ ███
███▄▄ ▀▀█▄█▀▄▀█▀█ ▀  ██▄█ ▀▄▄█▀▀▄▀▄▄▀█▀▄▀▄▄▄███
███▄▄█▄▀█▄▀ ▄▀█▀▀█▀▄▄ ▄███ ▄▄▄▄ ▀█▀▀  ▀  █ ████
███ ▄▀█ █▄ █ █  ▀ ▄▄ █▀▀▄▀▄▄ ▄█▄ ▄▀▄██▄█▀▀▀████
█████▄ ▄ ▄█▄▀  ▀ ▄▀▀▀▄█▄▀▄█▀ ▀ ▀   ▀▀ ▀█  ▄████
███▄▄████▄▄ ██ ▀ █  ██ █ █ ▄ █▀ ▀ ▀ ▄▄▄  ▀  ███
███ ▄▄▄▄▄ █▄██▀█▄▀ ▀ ▀█▀█▀ ▄█▀█▄█▄█ █▄█  █▀▀███
███ █   █ █▄██▀█  ▀██▄ █▀▄▄▄▄▄▄ ▀▀█  ▄ ▄███▄███
███ █▄▄▄█ ███▄█▄█▄ █  █▄█▀▄██▀▄▄▀ ▀▄ ███▄   ███
███▄▄▄▄▄▄▄█▄█▄█▄▄▄████▄▄█▄▄███▄▄▄█▄██▄█▄▄▄▄████
███████████████████████████████████████████████";

        assert!(address.qr_code_text().unwrap().contains(expected.trim()));

        assert_eq!(address.qr_code_uri(None).unwrap(), "data:image/bmp;base64,Qk2GAQAAAAAAAD4AAAAoAAAAKQAAACkAAAABAAEAAAAAAEgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+rhsdLwAAAIIBYidDgAAAuitpGseAAAC6FxQO0AAAALqGM/j4gAAAghPrII2AAAD+hUGKrAAAAACdlV+PgAAAw5WVyv2AAAAUfcT/9gAAAD62KlcnAAAAqV5aRQcAAADLW8XukAAAAAmtIQ39AAAA0sDx+G0AAAA4q8MaVAAAAOJCysWLgAAAQFCHbgKAAAB2Pxvq2oAAAMT876hGgAAA2ueBU1MAAAC4AQzPZYAAAI6ot+xlgAAA0fxBqruAAADX4QbxQAAAAKgn3wI9AAAA9mvTjNQAAADhUNCr54AAANcOWlNNAAAAxKq3TqUAAACnH0+yiIAAAFi4oJQIAAAAi8J7NXyAAAAAvg4kAAAAAP6qqqq/gAAAgtYuIaCAAAC6/AzSLoAAALrgXA4ugAAAuiJqsa6AAACCIz/toIAAAP7clm2/gAAA");
    }
}
