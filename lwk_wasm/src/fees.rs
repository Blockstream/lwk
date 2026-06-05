use std::collections::HashMap;

use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

use crate::Error;

/// The total fee paid by the transaction for each asset type.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Fees {
    inner: HashMap<lwk_wollet::elements::AssetId, u64>,
}

impl From<HashMap<lwk_wollet::elements::AssetId, u64>> for Fees {
    fn from(inner: HashMap<lwk_wollet::elements::AssetId, u64>) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl Fees {
    /// Returns the fees as a JavaScript `Map` of asset id to amount.
    #[wasm_bindgen]
    pub fn entries(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);

        Ok(self.inner.serialize(&serializer)?)
    }

    /// Note: the amounts are strings since `JSON.stringify` cannot handle `BigInt`s.
    /// Use `entries()` to get the raw data.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_maps_as_objects(true);
        Ok(self
            .inner
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<HashMap<String, String>>()
            .serialize(&serializer)?)
    }

    #[wasm_bindgen(js_name = toString)]
    /// Return the string representation of the fee.
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("contain simple types")
    }
}
