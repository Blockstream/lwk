use std::str::FromStr;

use crate::Error;
use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

/// A valid asset identifier. wrapper of [`elements::AssetId`]
///
/// 32 bytes encoded as hex string.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy)]
pub struct AssetId {
    inner: elements::AssetId,
}

impl std::fmt::Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<elements::AssetId> for AssetId {
    fn from(inner: elements::AssetId) -> Self {
        AssetId { inner }
    }
}

impl From<AssetId> for elements::AssetId {
    fn from(value: AssetId) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl AssetId {
    /// Creates an `AssetId`
    #[wasm_bindgen(constructor)]
    pub fn new(asset_id: &str) -> Result<AssetId, Error> {
        Ok(elements::AssetId::from_str(asset_id)?.into())
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    use crate::AssetId;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_asset_id() {
        let expected = "HexToArray(InvalidLength(InvalidLengthError { expected: 64, invalid: 2 }))";
        let hex = "xx";
        assert_eq!(expected, format!("{:?}", AssetId::new(hex).unwrap_err()));

        let expected = "HexToArray(InvalidChar(InvalidCharError { invalid: 120 }))";
        let hex = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert_eq!(expected, format!("{:?}", AssetId::new(hex).unwrap_err()));

        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        assert_eq!(hex, AssetId::new(hex).unwrap().to_string());
    }
}
