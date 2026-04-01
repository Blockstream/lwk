use crate::wallet_abi::*;

/// A canonical external taproot handle used by Wallet ABI bindings.
#[derive(uniffi::Object, Clone, Debug, PartialEq)]
#[uniffi::export(Display)]
pub struct WalletAbiTaprootHandle {
    pub(crate) inner: TaprootPubkeyGen,
}

impl std::fmt::Display for WalletAbiTaprootHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl WalletAbiTaprootHandle {
    /// Parse the canonical `<seed_or_ext-xonly_hex>:<pubkey>:<address>` taproot-handle string.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: TaprootPubkeyGen::from_str(s)?,
        }))
    }
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
/// The issuance kind for a Wallet ABI input issuance.
pub enum WalletAbiInputIssuanceKind {
    /// Create a new asset issuance.
    New,
    /// Reissue an existing asset.
    Reissue,
}

/// An asset selector for wallet-funded inputs.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiAssetFilter {
    pub(crate) inner: abi::AssetFilter,
}

#[uniffi::export]
impl WalletAbiAssetFilter {
    /// Build the Wallet ABI `none` asset filter variant.
    #[uniffi::constructor]
    pub fn none() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetFilter::None,
        })
    }

    /// Build the Wallet ABI `exact` asset filter variant.
    #[uniffi::constructor]
    pub fn exact(asset_id: AssetId) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetFilter::Exact {
                asset_id: asset_id.into(),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AssetFilter::None => "none",
            abi::AssetFilter::Exact { .. } => "exact",
        }
        .into()
    }

    /// Return the asset id when this filter is the `exact` variant.
    pub fn exact_asset_id(&self) -> Option<AssetId> {
        match self.inner {
            abi::AssetFilter::Exact { asset_id } => Some(asset_id.into()),
            abi::AssetFilter::None => None,
        }
    }
}

/// An amount selector for wallet-funded inputs.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiAmountFilter {
    pub(crate) inner: abi::AmountFilter,
}

#[uniffi::export]
impl WalletAbiAmountFilter {
    /// Build the Wallet ABI `none` amount filter variant.
    #[uniffi::constructor]
    pub fn none() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AmountFilter::None,
        })
    }

    /// Build the Wallet ABI `exact` amount filter variant.
    #[uniffi::constructor]
    pub fn exact(amount_sat: u64) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AmountFilter::Exact { amount_sat },
        })
    }

    /// Build the Wallet ABI `min` amount filter variant.
    #[uniffi::constructor]
    pub fn min(amount_sat: u64) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AmountFilter::Min { amount_sat },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AmountFilter::None => "none",
            abi::AmountFilter::Exact { .. } => "exact",
            abi::AmountFilter::Min { .. } => "min",
        }
        .into()
    }

    /// Return the selected amount for `exact` and `min` variants.
    pub fn amount_sat(&self) -> Option<u64> {
        match self.inner {
            abi::AmountFilter::Exact { amount_sat } | abi::AmountFilter::Min { amount_sat } => {
                Some(amount_sat)
            }
            abi::AmountFilter::None => None,
        }
    }
}

/// A lock selector for wallet-funded inputs.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiLockFilter {
    pub(crate) inner: abi::LockFilter,
}

#[uniffi::export]
impl WalletAbiLockFilter {
    /// Build the Wallet ABI `none` lock filter variant.
    #[uniffi::constructor]
    pub fn none() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::LockFilter::None,
        })
    }

    /// Build the Wallet ABI `script` lock filter variant.
    #[uniffi::constructor]
    pub fn script(script: &Script) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::LockFilter::Script {
                script: script.into(),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::LockFilter::None => "none",
            abi::LockFilter::Script { .. } => "script",
        }
        .into()
    }

    /// Return the script when this filter is the `script` variant.
    pub fn script_value(&self) -> Option<Arc<Script>> {
        match &self.inner {
            abi::LockFilter::Script { script } => Some(Arc::new(script.clone().into())),
            abi::LockFilter::None => None,
        }
    }
}

/// A grouped filter for selecting wallet-funded UTXOs.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiWalletSourceFilter {
    pub(crate) inner: abi::WalletSourceFilter,
}

#[uniffi::export]
impl WalletAbiWalletSourceFilter {
    /// Build a wallet source filter from asset, amount, and lock filters.
    #[uniffi::constructor]
    pub fn new(
        asset: &WalletAbiAssetFilter,
        amount: &WalletAbiAmountFilter,
        lock: &WalletAbiLockFilter,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::WalletSourceFilter {
                asset: asset.inner.clone(),
                amount: amount.inner.clone(),
                lock: lock.inner.clone(),
            },
        })
    }

    /// Build a wallet source filter that matches any wallet UTXO.
    #[uniffi::constructor]
    pub fn any() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::WalletSourceFilter::default(),
        })
    }

    /// Return the asset filter component.
    pub fn asset(&self) -> Arc<WalletAbiAssetFilter> {
        Arc::new(WalletAbiAssetFilter {
            inner: self.inner.asset.clone(),
        })
    }

    /// Return the amount filter component.
    pub fn amount(&self) -> Arc<WalletAbiAmountFilter> {
        Arc::new(WalletAbiAmountFilter {
            inner: self.inner.amount.clone(),
        })
    }

    /// Return the lock filter component.
    pub fn lock(&self) -> Arc<WalletAbiLockFilter> {
        Arc::new(WalletAbiLockFilter {
            inner: self.inner.lock.clone(),
        })
    }
}

/// A Wallet ABI input UTXO source.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiUtxoSource {
    pub(crate) inner: abi::UTXOSource,
}

#[uniffi::export]
impl WalletAbiUtxoSource {
    /// Build the Wallet ABI `wallet` UTXO source variant.
    #[uniffi::constructor]
    pub fn wallet(filter: &WalletAbiWalletSourceFilter) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::UTXOSource::Wallet {
                filter: filter.inner.clone(),
            },
        })
    }

    /// Build the Wallet ABI `provided` UTXO source variant.
    #[uniffi::constructor]
    pub fn provided(outpoint: &OutPoint) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::UTXOSource::Provided {
                outpoint: outpoint.into(),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::UTXOSource::Wallet { .. } => "wallet",
            abi::UTXOSource::Provided { .. } => "provided",
        }
        .into()
    }

    /// Return the wallet filter when this source is the `wallet` variant.
    pub fn wallet_filter(&self) -> Option<Arc<WalletAbiWalletSourceFilter>> {
        match &self.inner {
            abi::UTXOSource::Wallet { filter } => Some(Arc::new(WalletAbiWalletSourceFilter {
                inner: filter.clone(),
            })),
            abi::UTXOSource::Provided { .. } => None,
        }
    }

    /// Return the outpoint when this source is the `provided` variant.
    pub fn provided_outpoint(&self) -> Option<Arc<OutPoint>> {
        match self.inner {
            abi::UTXOSource::Provided { outpoint } => Some(Arc::new(outpoint.into())),
            abi::UTXOSource::Wallet { .. } => None,
        }
    }
}

/// Wallet ABI issuance data attached to an input.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiInputIssuance {
    pub(crate) inner: abi::InputIssuance,
}

#[uniffi::export]
impl WalletAbiInputIssuance {
    /// Build an input issuance object.
    #[uniffi::constructor]
    pub fn new(
        kind: WalletAbiInputIssuanceKind,
        asset_amount_sat: u64,
        token_amount_sat: u64,
        entropy: &[u8],
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::InputIssuance {
                kind: kind.into(),
                asset_amount_sat,
                token_amount_sat,
                entropy: entropy.try_into()?,
            },
        }))
    }

    /// Return the issuance kind.
    pub fn kind(&self) -> WalletAbiInputIssuanceKind {
        self.inner.kind.clone().into()
    }

    /// Return the issued asset amount in satoshi.
    pub fn asset_amount_sat(&self) -> u64 {
        self.inner.asset_amount_sat
    }

    /// Return the issued token amount in satoshi.
    pub fn token_amount_sat(&self) -> u64 {
        self.inner.token_amount_sat
    }

    /// Return the issuance entropy bytes.
    pub fn entropy(&self) -> Vec<u8> {
        self.inner.entropy.to_vec()
    }
}

/// The internal key source used by a Wallet ABI finalizer.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiInternalKeySource {
    pub(crate) inner: abi::InternalKeySource,
}

#[uniffi::export]
impl WalletAbiInternalKeySource {
    /// Build the Wallet ABI `bip0341` internal key source variant.
    #[uniffi::constructor]
    pub fn bip0341() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InternalKeySource::Bip0341,
        })
    }

    /// Build the Wallet ABI `external` internal key source variant.
    #[uniffi::constructor]
    pub fn external(handle: &WalletAbiTaprootHandle) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InternalKeySource::External {
                key: Box::new(handle.inner.clone()),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::InternalKeySource::Bip0341 => "bip0341",
            abi::InternalKeySource::External { .. } => "external",
        }
        .into()
    }

    /// Return the external handle when this source is the `external` variant.
    pub fn external_handle(&self) -> Option<Arc<WalletAbiTaprootHandle>> {
        match &self.inner {
            abi::InternalKeySource::External { key } => Some(Arc::new(WalletAbiTaprootHandle {
                inner: (**key).clone(),
            })),
            abi::InternalKeySource::Bip0341 => None,
        }
    }
}

/// A finalizer specification for a Wallet ABI input or output lock.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiFinalizerSpec {
    pub(crate) inner: abi::FinalizerSpec,
}

#[uniffi::export]
impl WalletAbiFinalizerSpec {
    /// Build the Wallet ABI `wallet` finalizer variant.
    #[uniffi::constructor]
    pub fn wallet() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::FinalizerSpec::Wallet,
        })
    }

    /// Build the Wallet ABI `simf` finalizer variant.
    ///
    /// `arguments` and `witness` must contain ABI-serialized Simplicity payload bytes.
    #[uniffi::constructor]
    pub fn simf(
        source_simf: &str,
        internal_key: &WalletAbiInternalKeySource,
        arguments: &[u8],
        witness: &[u8],
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::FinalizerSpec::Simf {
                source_simf: source_simf.to_string(),
                internal_key: internal_key.inner.clone(),
                arguments: arguments.to_vec(),
                witness: witness.to_vec(),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::FinalizerSpec::Wallet => "wallet",
            abi::FinalizerSpec::Simf { .. } => "simf",
        }
        .into()
    }

    /// Return the Simplicity source program when this finalizer is the `simf` variant.
    pub fn source_simf(&self) -> Option<String> {
        match &self.inner {
            abi::FinalizerSpec::Simf { source_simf, .. } => Some(source_simf.clone()),
            abi::FinalizerSpec::Wallet => None,
        }
    }

    /// Return the internal key source when this finalizer is the `simf` variant.
    pub fn internal_key(&self) -> Option<Arc<WalletAbiInternalKeySource>> {
        match &self.inner {
            abi::FinalizerSpec::Simf { internal_key, .. } => {
                Some(Arc::new(WalletAbiInternalKeySource {
                    inner: internal_key.clone(),
                }))
            }
            abi::FinalizerSpec::Wallet => None,
        }
    }

    /// Return ABI-serialized Simplicity arguments for the `simf` variant.
    pub fn arguments(&self) -> Option<Vec<u8>> {
        match &self.inner {
            abi::FinalizerSpec::Simf { arguments, .. } => Some(arguments.clone()),
            abi::FinalizerSpec::Wallet => None,
        }
    }

    /// Return ABI-serialized Simplicity witness bytes for the `simf` variant.
    pub fn witness(&self) -> Option<Vec<u8>> {
        match &self.inner {
            abi::FinalizerSpec::Simf { witness, .. } => Some(witness.clone()),
            abi::FinalizerSpec::Wallet => None,
        }
    }
}

/// The input unblinding strategy for a Wallet ABI input.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiInputUnblinding {
    pub(crate) inner: abi::InputUnblinding,
}

#[uniffi::export]
impl WalletAbiInputUnblinding {
    /// Build the Wallet ABI `wallet` input unblinding variant.
    #[uniffi::constructor]
    pub fn wallet() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InputUnblinding::Wallet,
        })
    }

    /// Build the Wallet ABI `provided` input unblinding variant.
    #[uniffi::constructor]
    pub fn provided(secret_key: &SecretKey) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InputUnblinding::Provided {
                secret_key: secret_key.into(),
            },
        })
    }

    /// Build the Wallet ABI `explicit` input unblinding variant.
    #[uniffi::constructor]
    pub fn explicit() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InputUnblinding::Explicit,
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::InputUnblinding::Wallet => "wallet",
            abi::InputUnblinding::Provided { .. } => "provided",
            abi::InputUnblinding::Explicit => "explicit",
        }
        .into()
    }

    /// Return the secret key when this unblinding mode is the `provided` variant.
    pub fn provided_secret_key(&self) -> Option<Arc<SecretKey>> {
        match self.inner {
            abi::InputUnblinding::Provided { secret_key } => {
                Some(Arc::new(SecretKey::from(secret_key)))
            }
            abi::InputUnblinding::Wallet | abi::InputUnblinding::Explicit => None,
        }
    }
}

/// A Wallet ABI input schema entry.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiInputSchema {
    pub(crate) inner: abi::InputSchema,
}

#[uniffi::export]
impl WalletAbiInputSchema {
    /// Build an input schema from a typed sequence wrapper.
    #[uniffi::constructor]
    pub fn new(
        id: &str,
        utxo_source: &WalletAbiUtxoSource,
        unblinding: &WalletAbiInputUnblinding,
        sequence: &TxSequence,
        finalizer: &WalletAbiFinalizerSpec,
    ) -> Arc<Self> {
        Self::new_with_sequence_consensus(
            id,
            utxo_source,
            unblinding,
            sequence.to_consensus_u32(),
            finalizer,
        )
    }

    /// Build an input schema from a raw consensus sequence number.
    #[uniffi::constructor]
    pub fn new_with_sequence_consensus(
        id: &str,
        utxo_source: &WalletAbiUtxoSource,
        unblinding: &WalletAbiInputUnblinding,
        sequence_consensus_u32: u32,
        finalizer: &WalletAbiFinalizerSpec,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::InputSchema {
                id: id.to_string(),
                utxo_source: utxo_source.inner.clone(),
                unblinding: unblinding.inner.clone(),
                sequence: elements::Sequence::from_consensus(sequence_consensus_u32),
                issuance: None,
                finalizer: finalizer.inner.clone(),
            },
        })
    }

    /// Return a copy of this input schema with issuance data attached.
    pub fn with_issuance(&self, issuance: &WalletAbiInputIssuance) -> Arc<Self> {
        let mut inner = self.inner.clone();
        inner.issuance = Some(issuance.inner.clone());
        Arc::new(Self { inner })
    }

    /// Return the input identifier.
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Return the input UTXO source.
    pub fn utxo_source(&self) -> Arc<WalletAbiUtxoSource> {
        Arc::new(WalletAbiUtxoSource {
            inner: self.inner.utxo_source.clone(),
        })
    }

    /// Return the input unblinding strategy.
    pub fn unblinding(&self) -> Arc<WalletAbiInputUnblinding> {
        Arc::new(WalletAbiInputUnblinding {
            inner: self.inner.unblinding.clone(),
        })
    }

    /// Return the input sequence.
    pub fn sequence(&self) -> Arc<TxSequence> {
        Arc::new(self.inner.sequence.into())
    }

    /// Return the issuance data when this input carries an issuance.
    pub fn issuance(&self) -> Option<Arc<WalletAbiInputIssuance>> {
        self.inner.issuance.as_ref().map(|issuance| {
            Arc::new(WalletAbiInputIssuance {
                inner: issuance.clone(),
            })
        })
    }

    /// Return the input finalizer specification.
    pub fn finalizer(&self) -> Arc<WalletAbiFinalizerSpec> {
        Arc::new(WalletAbiFinalizerSpec {
            inner: self.inner.finalizer.clone(),
        })
    }
}
