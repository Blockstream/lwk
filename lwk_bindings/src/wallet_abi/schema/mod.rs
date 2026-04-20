//! Typed Wallet ABI transport bindings for request and response schema data.

use serde::de::DeserializeOwned;
use serde_json::Value;

pub mod capabilities;
pub mod evaluate;
pub mod filters;
pub mod outputs;
pub mod preview;
pub mod roots;
pub mod simf;

pub(crate) fn from_json_compat<T>(json: &str) -> Result<T, crate::LwkError>
where
    T: DeserializeOwned,
{
    match serde_json::from_str(json) {
        Ok(decoded) => Ok(decoded),
        Err(primary_error) => {
            let mut value = match serde_json::from_str::<Value>(json) {
                Ok(value) => value,
                Err(_) => return Err(primary_error.into()),
            };

            if !rewrite_top_level_legacy_network_alias(&mut value)? {
                return Err(primary_error.into());
            }

            Ok(serde_json::from_value(value)?)
        }
    }
}

fn rewrite_top_level_legacy_network_alias(value: &mut Value) -> Result<bool, crate::LwkError> {
    let Value::Object(map) = value else {
        return Ok(false);
    };

    let Some(network) = map.get_mut("network") else {
        return Ok(false);
    };

    let Some(name) = network.as_str() else {
        return Ok(false);
    };

    *network = match name {
        "liquid" => Value::String("Liquid".to_string()),
        "testnet-liquid" => Value::String("LiquidTestnet".to_string()),
        "localtest-liquid" => serde_json::to_value(lwk_wollet::ElementsNetwork::default_regtest())?,
        _ => return Ok(false),
    };

    Ok(true)
}
