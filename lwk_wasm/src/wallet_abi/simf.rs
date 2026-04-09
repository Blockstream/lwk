use crate::{Error, SimplicityArguments};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A runtime-resolved argument directive for Wallet ABI `SimfArguments`.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiRuntimeSimfValue {
    inner: abi::RuntimeSimfValue,
}

#[wasm_bindgen]
impl WalletAbiRuntimeSimfValue {
    /// Build the Wallet ABI `new_issuance_asset` runtime argument variant.
    #[wasm_bindgen(js_name = newIssuanceAsset)]
    pub fn new_issuance_asset(input_index: u32) -> WalletAbiRuntimeSimfValue {
        Self {
            inner: abi::RuntimeSimfValue::NewIssuanceAsset { input_index },
        }
    }

    /// Build the Wallet ABI `new_issuance_token` runtime argument variant.
    #[wasm_bindgen(js_name = newIssuanceToken)]
    pub fn new_issuance_token(input_index: u32) -> WalletAbiRuntimeSimfValue {
        Self {
            inner: abi::RuntimeSimfValue::NewIssuanceToken { input_index },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::RuntimeSimfValue::NewIssuanceAsset { .. } => "new_issuance_asset",
            abi::RuntimeSimfValue::NewIssuanceToken { .. } => "new_issuance_token",
        }
        .to_string()
    }

    /// Return the referenced input index.
    #[wasm_bindgen(js_name = inputIndex)]
    pub fn input_index(&self) -> u32 {
        match self.inner {
            abi::RuntimeSimfValue::NewIssuanceAsset { input_index }
            | abi::RuntimeSimfValue::NewIssuanceToken { input_index } => input_index,
        }
    }
}

/// A typed Wallet ABI `FinalizerSpec::Simf.arguments` payload builder.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiSimfArguments {
    inner: abi::SimfArguments,
}

#[wasm_bindgen]
impl WalletAbiSimfArguments {
    /// Build an arguments payload from static Simplicity arguments only.
    #[wasm_bindgen(js_name = fromResolved)]
    pub fn from_resolved(
        resolved: &SimplicityArguments,
    ) -> Result<WalletAbiSimfArguments, Error> {
        Ok(Self {
            inner: abi::SimfArguments::new(resolved.to_inner()?),
        })
    }

    /// Parse an arguments payload from Wallet ABI bytes.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<WalletAbiSimfArguments, Error> {
        abi::deserialize_arguments(bytes)
            .map(|inner| Self { inner })
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Return a copy of this arguments payload with one runtime directive added or replaced.
    #[wasm_bindgen(js_name = appendRuntimeArgument)]
    pub fn append_runtime_argument(
        &self,
        name: &str,
        runtime_argument: &WalletAbiRuntimeSimfValue,
    ) -> WalletAbiSimfArguments {
        let mut inner = self.inner.clone();
        inner.append_runtime_simf_value(name, runtime_argument.inner.clone());
        Self { inner }
    }

    /// Return the runtime argument names.
    #[wasm_bindgen(js_name = runtimeArgumentNames)]
    pub fn runtime_argument_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.inner.runtime_arguments.keys().cloned().collect();
        names.sort();
        names
    }

    /// Return the runtime argument for `name` when present.
    #[wasm_bindgen(js_name = runtimeArgument)]
    pub fn runtime_argument(&self, name: &str) -> Option<WalletAbiRuntimeSimfValue> {
        self.inner
            .runtime_arguments
            .get(name)
            .cloned()
            .map(|inner| WalletAbiRuntimeSimfValue { inner })
    }

    /// Serialize this arguments payload into Wallet ABI bytes.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        abi::serialize_arguments(&self.inner).map_err(|error| Error::Generic(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiRuntimeSimfValue, WalletAbiSimfArguments};

    use crate::{SimplicityArguments, SimplicityTypedValue};

    #[test]
    fn wallet_abi_simf_arguments_roundtrip() {
        let arguments =
            WalletAbiSimfArguments::from_resolved(&SimplicityArguments::new().add_value(
                "a",
                &SimplicityTypedValue::from_u8(42),
            ))
            .expect("arguments");
        let with_runtime = arguments.append_runtime_argument(
            "issuance_asset",
            &WalletAbiRuntimeSimfValue::new_issuance_asset(1),
        );

        let encoded = with_runtime.to_bytes().expect("serialize arguments");
        let decoded = WalletAbiSimfArguments::from_bytes(&encoded).expect("deserialize arguments");

        assert_eq!(decoded.runtime_argument_names(), vec!["issuance_asset".to_string()]);
        assert_eq!(
            decoded
                .runtime_argument("issuance_asset")
                .expect("runtime argument")
                .kind(),
            "new_issuance_asset"
        );
        assert_eq!(
            decoded
                .runtime_argument("issuance_asset")
                .expect("runtime argument")
                .input_index(),
            1
        );
    }
}
