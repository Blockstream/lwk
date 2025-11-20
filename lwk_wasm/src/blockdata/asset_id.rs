use std::{
    collections::{BTreeSet, HashSet},
    str::FromStr,
};

use crate::Error;
use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

/// A valid asset identifier.
///
/// 32 bytes encoded as hex string.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy)]
pub struct AssetId {
    inner: elements::AssetId,
}

// wasm_bindgen does not support Vec<T> as a wrapper of Vec<T>
/// An ordered collection of asset identifiers.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub struct AssetIds {
    inner: BTreeSet<elements::AssetId>,
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

impl From<AssetIds> for Vec<elements::AssetId> {
    fn from(value: AssetIds) -> Self {
        value.inner.into_iter().collect()
    }
}

impl From<AssetIds> for Vec<AssetId> {
    fn from(value: AssetIds) -> Self {
        value.inner.into_iter().map(AssetId::from).collect()
    }
}

impl From<&AssetIds> for Vec<elements::AssetId> {
    fn from(value: &AssetIds) -> Self {
        value.inner.clone().into_iter().collect()
    }
}

impl From<Vec<elements::AssetId>> for AssetIds {
    fn from(value: Vec<elements::AssetId>) -> Self {
        AssetIds {
            inner: value.into_iter().collect(),
        }
    }
}

impl From<HashSet<elements::AssetId>> for AssetIds {
    fn from(value: HashSet<elements::AssetId>) -> Self {
        AssetIds {
            inner: value.into_iter().collect(),
        }
    }
}

impl From<Vec<AssetId>> for AssetIds {
    fn from(value: Vec<AssetId>) -> Self {
        AssetIds {
            inner: value.into_iter().map(|asset_id| asset_id.inner).collect(),
        }
    }
}

impl std::fmt::Display for AssetIds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

#[wasm_bindgen]
impl AssetId {
    /// Creates an `AssetId`
    #[wasm_bindgen(constructor)]
    pub fn new(asset_id: &str) -> Result<AssetId, Error> {
        Ok(elements::AssetId::from_str(asset_id)?.into())
    }

    /// Return the string representation of the asset identifier (64 hex characters).
    /// This representation can be used to recreate the asset identifier via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

#[wasm_bindgen]
impl AssetIds {
    /// Return an empty list of asset identifiers.
    pub fn empty() -> Result<AssetIds, Error> {
        Ok(AssetIds {
            inner: BTreeSet::new(),
        })
    }

    /// Return the string representation of this list of asset identifiers.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    // TODO: implement entries()
}
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    use crate::{AssetId, AssetIds};

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

    #[wasm_bindgen_test]
    async fn test_asset_ids() {
        let a = "0000000000000000000000000000000000000000000000000000000000000001";
        let asset_id1 = AssetId::new(a).unwrap();
        let b = "0000000000000000000000000000000000000000000000000000000000000002";
        let asset_id2 = AssetId::new(b).unwrap();
        let c = "0000000000000000000000000000000000000000000000000000000000000003";
        let asset_id3 = AssetId::new(c).unwrap();

        let asset_ids: AssetIds = vec![asset_id3, asset_id1, asset_id2].into();
        assert_eq!(asset_ids.to_string(), format!("{{{a}, {b}, {c}}}"));
        let asset_ids2: AssetIds = vec![asset_id2, asset_id1, asset_id3].into();
        assert_eq!(asset_ids, asset_ids2);
    }
}
