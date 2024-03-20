use wasm_bindgen::prelude::*;

use crate::Error;

/// Wrapper of [`lwk_wollet::Update`]
#[wasm_bindgen]
#[derive(Clone, PartialEq, Eq)]
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

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        Ok(self.inner.serialize()?)
    }
}

#[cfg(test)]
mod tests {
    use lwk_wollet::hashes::hex::FromHex;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    pub fn update_test_vector_bytes() -> Vec<u8> {
        Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
    }

    #[wasm_bindgen_test]
    fn test_update() {
        let bytes = update_test_vector_bytes();
        let update = crate::Update::new(&bytes).unwrap();
        assert_eq!(update.serialize().unwrap(), bytes);
    }
}
