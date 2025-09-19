use std::collections::BTreeMap;

use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

use crate::Error;

/// A signed balance of assets, to represent a balance with negative values such
/// as the results of a transaction from the perspective of a wallet.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Balance {
    inner: lwk_common::SignedBalance,
}

impl From<lwk_common::SignedBalance> for Balance {
    fn from(inner: lwk_common::SignedBalance) -> Self {
        Self { inner }
    }
}

impl From<lwk_common::Balance> for Balance {
    fn from(balance: lwk_common::Balance) -> Self {
        // Convert Balance to SignedBalance by mapping positive values
        let signed_map: std::collections::BTreeMap<lwk_wollet::elements::AssetId, i64> = balance
            .iter()
            .map(|(asset_id, amount)| (*asset_id, *amount as i64))
            .collect();
        Self {
            inner: lwk_common::SignedBalance::from(signed_map),
        }
    }
}

#[wasm_bindgen]
impl Balance {
    /// Convert the balance to a JsValue for serialization
    ///
    /// Note: the amounts are strings since `JSON.stringify` cannot handle `BigInt`s.
    /// Use `entries()` to get the raw data.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_maps_as_objects(true);

        Ok(self
            .inner
            .as_ref()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<String, String>>()
            .serialize(&serializer)?)
    }

    /// Returns the balance as an array of [key, value] pairs.
    #[wasm_bindgen]
    pub fn entries(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);

        Ok(self.inner.serialize(&serializer)?)
    }

    /// Return the string representation of the balance.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("contain simple types")
    }
}
