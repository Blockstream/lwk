use crate::wallet_abi::*;

/// A runtime-resolved argument directive for Wallet ABI `SimfArguments`.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiRuntimeSimfValue {
    pub(crate) inner: abi::RuntimeSimfValue,
}

#[uniffi::export]
impl WalletAbiRuntimeSimfValue {
    /// Build the Wallet ABI `new_issuance_asset` runtime argument variant.
    #[uniffi::constructor]
    pub fn new_issuance_asset(input_index: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::RuntimeSimfValue::NewIssuanceAsset { input_index },
        })
    }

    /// Build the Wallet ABI `new_issuance_token` runtime argument variant.
    #[uniffi::constructor]
    pub fn new_issuance_token(input_index: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::RuntimeSimfValue::NewIssuanceToken { input_index },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::RuntimeSimfValue::NewIssuanceAsset { .. } => "new_issuance_asset",
            abi::RuntimeSimfValue::NewIssuanceToken { .. } => "new_issuance_token",
        }
        .into()
    }

    /// Return the referenced input index.
    pub fn input_index(&self) -> u32 {
        match self.inner {
            abi::RuntimeSimfValue::NewIssuanceAsset { input_index }
            | abi::RuntimeSimfValue::NewIssuanceToken { input_index } => input_index,
        }
    }
}

/// A typed Wallet ABI `FinalizerSpec::Simf.arguments` payload builder.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiSimfArguments {
    pub(crate) inner: abi::SimfArguments,
}

#[uniffi::export]
impl WalletAbiSimfArguments {
    /// Build an arguments payload from static Simplicity arguments only.
    #[uniffi::constructor]
    pub fn new(resolved: &SimplicityArguments) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::SimfArguments::new(resolved.to_inner()?),
        }))
    }

    /// Return a copy of this arguments payload with one runtime directive added or replaced.
    pub fn append_runtime_argument(
        &self,
        name: &str,
        runtime_argument: &WalletAbiRuntimeSimfValue,
    ) -> Arc<Self> {
        let mut inner = self.inner.clone();
        inner.append_runtime_simf_value(name, runtime_argument.inner.clone());
        Arc::new(Self { inner })
    }

    /// Return the runtime argument names.
    pub fn runtime_argument_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.inner.runtime_arguments.keys().cloned().collect();
        names.sort();
        names
    }

    /// Return the runtime argument for `name` when present.
    pub fn runtime_argument(&self, name: &str) -> Option<Arc<WalletAbiRuntimeSimfValue>> {
        self.inner
            .runtime_arguments
            .get(name)
            .cloned()
            .map(|inner| Arc::new(WalletAbiRuntimeSimfValue { inner }))
    }

    /// Serialize this arguments payload into Wallet ABI bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LwkError> {
        abi::serialize_arguments(&self.inner).map_err(Into::into)
    }
}

/// A runtime-resolved witness directive for Wallet ABI `SimfWitness`.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiRuntimeSimfWitness {
    pub(crate) inner: abi::RuntimeSimfWitness,
}

#[uniffi::export]
impl WalletAbiRuntimeSimfWitness {
    /// Build the Wallet ABI `sig_hash_all` runtime witness variant.
    #[uniffi::constructor]
    pub fn sig_hash_all(name: &str, public_key: &XOnlyPublicKey) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::RuntimeSimfWitness::SigHashAll {
                name: name.to_owned(),
                public_key: (*public_key).into(),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::RuntimeSimfWitness::SigHashAll { .. } => "sig_hash_all",
        }
        .into()
    }

    /// Return the runtime witness name.
    pub fn name(&self) -> String {
        match &self.inner {
            abi::RuntimeSimfWitness::SigHashAll { name, .. } => name.clone(),
        }
    }

    /// Return the x-only public key for the runtime witness.
    pub fn public_key(&self) -> Arc<XOnlyPublicKey> {
        match &self.inner {
            abi::RuntimeSimfWitness::SigHashAll { public_key, .. } => {
                Arc::new((*public_key).into())
            }
        }
    }
}

/// A typed Wallet ABI `FinalizerSpec::Simf.witness` payload builder.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiSimfWitness {
    pub(crate) inner: abi::SimfWitness,
}

#[uniffi::export]
impl WalletAbiSimfWitness {
    /// Build a witness payload from static Simplicity witness values only.
    #[uniffi::constructor]
    pub fn new(resolved: &SimplicityWitnessValues) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::SimfWitness::new(resolved.to_inner()?),
        }))
    }

    /// Build a witness payload from static witness values and runtime directives.
    #[uniffi::constructor]
    pub fn new_with_runtime_arguments(
        resolved: &SimplicityWitnessValues,
        runtime_arguments: &[Arc<WalletAbiRuntimeSimfWitness>],
    ) -> Result<Arc<Self>, LwkError> {
        let mut inner = abi::SimfWitness::new(resolved.to_inner()?);
        inner.runtime_arguments = runtime_arguments
            .iter()
            .map(|argument| argument.inner.clone())
            .collect();
        Ok(Arc::new(Self { inner }))
    }

    /// Return a copy of this witness payload with one runtime directive appended.
    pub fn append_runtime_argument(
        &self,
        runtime_argument: &WalletAbiRuntimeSimfWitness,
    ) -> Arc<Self> {
        let mut inner = self.inner.clone();
        inner.runtime_arguments.push(runtime_argument.inner.clone());
        Arc::new(Self { inner })
    }

    /// Return the runtime witness directives.
    pub fn runtime_arguments(&self) -> Vec<Arc<WalletAbiRuntimeSimfWitness>> {
        self.inner
            .runtime_arguments
            .iter()
            .cloned()
            .map(|inner| Arc::new(WalletAbiRuntimeSimfWitness { inner }))
            .collect()
    }

    /// Serialize this witness payload into Wallet ABI bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LwkError> {
        abi::serialize_witness(&self.inner).map_err(Into::into)
    }
}
