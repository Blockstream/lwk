//! Typed Wallet ABI schema wrappers for wasm consumers.

mod capabilities;
mod evaluate;
mod filters;
mod outputs;
mod preview;
mod roots;
mod simf;

use crate::Error;

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;

pub use capabilities::WalletAbiCapabilities;
pub use evaluate::{WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse};
pub use filters::{
    WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiFinalizerSpec, WalletAbiInputIssuance,
    WalletAbiInputIssuanceKind, WalletAbiInputSchema, WalletAbiInputUnblinding,
    WalletAbiInternalKeySource, WalletAbiLockFilter, WalletAbiTaprootHandle, WalletAbiUtxoSource,
    WalletAbiWalletSourceFilter,
};
pub use outputs::{
    WalletAbiAssetVariant, WalletAbiBlinderVariant, WalletAbiLockVariant, WalletAbiOutputSchema,
    WalletAbiRuntimeParams,
};
pub use preview::{
    WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput, WalletAbiPreviewOutputKind,
    WalletAbiRequestPreview,
};
pub use roots::{
    WalletAbiErrorInfo, WalletAbiStatus, WalletAbiTransactionInfo, WalletAbiTxCreateRequest,
    WalletAbiTxCreateResponse,
};
pub use simf::{
    WalletAbiRuntimeSimfValue, WalletAbiRuntimeSimfWitness, WalletAbiSimfArguments,
    WalletAbiSimfWitness,
};

pub(crate) fn json_from_js_value<T>(value: JsValue) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(json) = value.as_string() {
            return Ok(serde_json::from_str(&json)?);
        }
        serde_wasm_bindgen::from_value(value).map_err(Into::into)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = value;
        Err(Error::Generic(
            "fromJson() is only available on wasm32 targets".into(),
        ))
    }
}

pub(crate) fn js_value_from_json<T>(value: &T) -> Result<JsValue, Error>
where
    T: Serialize,
{
    #[cfg(target_arch = "wasm32")]
    {
        serde_wasm_bindgen::to_value(value).map_err(Into::into)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = value;
        Err(Error::Generic(
            "toJSON() is only available on wasm32 targets".into(),
        ))
    }
}
