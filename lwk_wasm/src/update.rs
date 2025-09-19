use wasm_bindgen::prelude::*;

use crate::{Error, Wollet, WolletDescriptor};

/// An Update contains the delta of information to be applied to the wallet to reach the latest status.
/// It's created passing a reference to the wallet to the blockchain client
#[wasm_bindgen]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Update {
    inner: lwk_wollet::Update,
}

impl From<lwk_wollet::Update> for Update {
    fn from(inner: lwk_wollet::Update) -> Self {
        Self { inner }
    }
}

impl From<Update> for lwk_wollet::Update {
    fn from(value: Update) -> Self {
        value.inner
    }
}

impl From<&Update> for lwk_wollet::Update {
    fn from(value: &Update) -> Self {
        value.inner.clone()
    }
}

impl AsRef<lwk_wollet::Update> for Update {
    fn as_ref(&self) -> &lwk_wollet::Update {
        &self.inner
    }
}

#[wasm_bindgen]
impl Update {
    /// Creates an `Update`
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Update, Error> {
        Ok(lwk_wollet::Update::deserialize(bytes)?.into())
    }

    /// Serialize an update to a byte array
    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        Ok(self.inner.serialize()?)
    }

    /// Serialize an update to a base64 encoded string,
    /// encrypted with a key derived from the descriptor.
    /// Decrypt using `deserialize_decrypted_base64()`
    #[wasm_bindgen(js_name = serializeEncryptedBase64)]
    pub fn serialize_encrypted_base64(&self, desc: &WolletDescriptor) -> Result<String, Error> {
        Ok(self.inner.serialize_encrypted_base64(desc.as_ref())?)
    }

    /// Deserialize an update from a base64 encoded string,
    /// decrypted with a key derived from the descriptor.
    /// Create the base64 using `serialize_encrypted_base64()`
    #[wasm_bindgen(js_name = deserializeDecryptedBase64)]
    pub fn deserialize_decrypted_base64(
        base64: &str,
        desc: &WolletDescriptor,
    ) -> Result<Update, Error> {
        Ok(lwk_wollet::Update::deserialize_decrypted_base64(base64, desc.as_ref())?.into())
    }

    /// Whether this update only changes the tip
    #[wasm_bindgen(js_name = onlyTip)]
    pub fn only_tip(&self) -> bool {
        self.inner.only_tip()
    }

    /// Prune the update, removing unneeded data from transactions.
    pub fn prune(&mut self, wollet: &Wollet) {
        self.inner.prune(wollet.as_ref());
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use lwk_wollet::hashes::hex::FromHex;
    use wasm_bindgen_test::*;

    use crate::{Update, WolletDescriptor};

    wasm_bindgen_test_configure!(run_in_browser);

    pub fn update_test_vector_bytes() -> Vec<u8> {
        Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
    }

    #[wasm_bindgen_test]
    fn test_update() {
        let bytes = update_test_vector_bytes();
        let update = crate::Update::new(&bytes).unwrap();
        // assert_eq!(update.serialize().unwrap(), bytes); // not true anymore because test vector is v0, backward comp tested upstream anyway
        assert!(!update.only_tip());

        let base64 = include_str!("../test_data/update.base64");
        let desc_str = include_str!("../test_data/desc");
        let desc = WolletDescriptor::new(desc_str).unwrap();
        assert_eq!(desc_str, desc.to_string());
        let update = Update::deserialize_decrypted_base64(base64, &desc).unwrap();
        let base64_back = update.serialize_encrypted_base64(&desc).unwrap();
        let update_back = Update::deserialize_decrypted_base64(&base64_back, &desc).unwrap();
        assert_eq!(update, update_back);
    }
}
