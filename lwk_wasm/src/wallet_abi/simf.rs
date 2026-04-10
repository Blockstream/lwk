use crate::{Error, SimplicityArguments, SimplicityWitnessValues, XOnlyPublicKey};

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
    pub fn from_resolved(resolved: &SimplicityArguments) -> Result<WalletAbiSimfArguments, Error> {
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

/// A runtime-resolved witness directive for Wallet ABI `SimfWitness`.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiRuntimeSimfWitness {
    inner: abi::RuntimeSimfWitness,
}

#[wasm_bindgen]
impl WalletAbiRuntimeSimfWitness {
    /// Build the Wallet ABI `sig_hash_all` runtime witness variant.
    #[wasm_bindgen(js_name = sigHashAll)]
    pub fn sig_hash_all(name: &str, public_key: &XOnlyPublicKey) -> WalletAbiRuntimeSimfWitness {
        Self {
            inner: abi::RuntimeSimfWitness::SigHashAll {
                name: name.to_string(),
                public_key: (*public_key).into(),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::RuntimeSimfWitness::SigHashAll { .. } => "sig_hash_all",
        }
        .to_string()
    }

    /// Return the runtime witness name.
    pub fn name(&self) -> String {
        match &self.inner {
            abi::RuntimeSimfWitness::SigHashAll { name, .. } => name.clone(),
        }
    }

    /// Return the x-only public key for the runtime witness.
    #[wasm_bindgen(js_name = publicKey)]
    pub fn public_key(&self) -> XOnlyPublicKey {
        match self.inner {
            abi::RuntimeSimfWitness::SigHashAll { public_key, .. } => public_key.into(),
        }
    }
}

/// A typed Wallet ABI `FinalizerSpec::Simf.witness` payload builder.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiSimfWitness {
    inner: abi::SimfWitness,
}

#[wasm_bindgen]
impl WalletAbiSimfWitness {
    /// Build a witness payload from static Simplicity witness values only.
    #[wasm_bindgen(js_name = fromResolved)]
    pub fn from_resolved(
        resolved: &SimplicityWitnessValues,
    ) -> Result<WalletAbiSimfWitness, Error> {
        Ok(Self {
            inner: abi::SimfWitness::new(resolved.to_inner()?),
        })
    }

    /// Parse a witness payload from Wallet ABI bytes.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<WalletAbiSimfWitness, Error> {
        abi::deserialize_witness(bytes)
            .map(|inner| Self { inner })
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Build a witness payload from static witness values and runtime directives.
    #[wasm_bindgen(js_name = newWithRuntimeArguments)]
    pub fn new_with_runtime_arguments(
        resolved: &SimplicityWitnessValues,
        runtime_arguments: Vec<WalletAbiRuntimeSimfWitness>,
    ) -> Result<WalletAbiSimfWitness, Error> {
        let mut inner = abi::SimfWitness::new(resolved.to_inner()?);
        inner.runtime_arguments = runtime_arguments
            .into_iter()
            .map(|argument| argument.inner)
            .collect();
        Ok(Self { inner })
    }

    /// Return a copy of this witness payload with one runtime directive appended.
    #[wasm_bindgen(js_name = appendRuntimeArgument)]
    pub fn append_runtime_argument(
        &self,
        runtime_argument: &WalletAbiRuntimeSimfWitness,
    ) -> WalletAbiSimfWitness {
        let mut inner = self.inner.clone();
        inner.runtime_arguments.push(runtime_argument.inner.clone());
        Self { inner }
    }

    /// Return the runtime witness directives.
    #[wasm_bindgen(js_name = runtimeArguments)]
    pub fn runtime_arguments(&self) -> Vec<WalletAbiRuntimeSimfWitness> {
        self.inner
            .runtime_arguments
            .iter()
            .cloned()
            .map(|inner| WalletAbiRuntimeSimfWitness { inner })
            .collect()
    }

    /// Serialize this witness payload into Wallet ABI bytes.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        abi::serialize_witness(&self.inner).map_err(|error| Error::Generic(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WalletAbiRuntimeSimfValue, WalletAbiRuntimeSimfWitness, WalletAbiSimfArguments,
        WalletAbiSimfWitness,
    };

    use crate::{
        SimplicityArguments, SimplicityTypedValue, SimplicityWitnessValues, XOnlyPublicKey,
    };

    #[test]
    fn wallet_abi_simf_arguments_roundtrip() {
        let arguments = WalletAbiSimfArguments::from_resolved(
            &SimplicityArguments::new().add_value("a", &SimplicityTypedValue::from_u8(42)),
        )
        .expect("arguments");
        let with_runtime = arguments.append_runtime_argument(
            "issuance_asset",
            &WalletAbiRuntimeSimfValue::new_issuance_asset(1),
        );

        let encoded = with_runtime.to_bytes().expect("serialize arguments");
        let decoded = WalletAbiSimfArguments::from_bytes(&encoded).expect("deserialize arguments");

        assert_eq!(
            decoded.runtime_argument_names(),
            vec!["issuance_asset".to_string()]
        );
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

    #[test]
    fn wallet_abi_simf_witness_roundtrip() {
        let public_key = XOnlyPublicKey::from_string(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("xonly key");
        let witness = WalletAbiSimfWitness::new_with_runtime_arguments(
            &SimplicityWitnessValues::new()
                .add_value("w0", &SimplicityTypedValue::from_boolean(true)),
            vec![WalletAbiRuntimeSimfWitness::sig_hash_all(
                "sig",
                &public_key,
            )],
        )
        .expect("witness");

        let encoded = witness.to_bytes().expect("serialize witness");
        let decoded = WalletAbiSimfWitness::from_bytes(&encoded).expect("deserialize witness");
        let runtime_argument = decoded
            .runtime_arguments()
            .into_iter()
            .next()
            .expect("runtime argument");

        assert_eq!(runtime_argument.kind(), "sig_hash_all");
        assert_eq!(runtime_argument.name(), "sig".to_string());
        assert_eq!(runtime_argument.public_key(), public_key);
    }
}
