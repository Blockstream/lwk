use crate::wallet_abi::schema::filters::{WalletAbiFinalizerSpec, WalletAbiInputSchema};
use crate::wallet_abi::*;

/// A Wallet ABI output lock variant.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiLockVariant {
    pub(crate) inner: abi::LockVariant,
}

#[uniffi::export]
impl WalletAbiLockVariant {
    /// Build the Wallet ABI `wallet` lock variant.
    #[uniffi::constructor]
    pub fn wallet() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::LockVariant::Wallet,
        })
    }

    /// Build the Wallet ABI `script` lock variant.
    #[uniffi::constructor]
    pub fn script(script: &Script) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::LockVariant::Script {
                script: script.into(),
            },
        })
    }

    /// Build the Wallet ABI `finalizer` lock variant.
    #[uniffi::constructor]
    pub fn finalizer(finalizer: &WalletAbiFinalizerSpec) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::LockVariant::Finalizer {
                finalizer: Box::new(finalizer.inner.clone()),
            },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::LockVariant::Wallet => "wallet",
            abi::LockVariant::Script { .. } => "script",
            abi::LockVariant::Finalizer { .. } => "finalizer",
        }
        .into()
    }

    /// Return the script when this lock is the `script` variant.
    pub fn script_value(&self) -> Option<Arc<Script>> {
        match &self.inner {
            abi::LockVariant::Script { script } => Some(Arc::new(script.clone().into())),
            abi::LockVariant::Wallet | abi::LockVariant::Finalizer { .. } => None,
        }
    }

    /// Return the finalizer when this lock is the `finalizer` variant.
    pub fn finalizer_value(&self) -> Option<Arc<WalletAbiFinalizerSpec>> {
        match &self.inner {
            abi::LockVariant::Finalizer { finalizer } => Some(Arc::new(WalletAbiFinalizerSpec {
                inner: (**finalizer).clone(),
            })),
            abi::LockVariant::Wallet | abi::LockVariant::Script { .. } => None,
        }
    }
}

/// A Wallet ABI output asset variant.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiAssetVariant {
    pub(crate) inner: abi::AssetVariant,
}

#[uniffi::export]
impl WalletAbiAssetVariant {
    /// Build the Wallet ABI `asset_id` asset variant.
    #[uniffi::constructor]
    pub fn asset_id(asset_id: AssetId) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetVariant::AssetId {
                asset_id: asset_id.into(),
            },
        })
    }

    /// Build the Wallet ABI `new_issuance_asset` asset variant.
    #[uniffi::constructor]
    pub fn new_issuance_asset(input_index: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetVariant::NewIssuanceAsset { input_index },
        })
    }

    /// Build the Wallet ABI `new_issuance_token` asset variant.
    #[uniffi::constructor]
    pub fn new_issuance_token(input_index: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetVariant::NewIssuanceToken { input_index },
        })
    }

    /// Build the Wallet ABI `re_issuance_asset` asset variant.
    #[uniffi::constructor]
    pub fn re_issuance_asset(input_index: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::AssetVariant::ReIssuanceAsset { input_index },
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AssetVariant::AssetId { .. } => "asset_id",
            abi::AssetVariant::NewIssuanceAsset { .. } => "new_issuance_asset",
            abi::AssetVariant::NewIssuanceToken { .. } => "new_issuance_token",
            abi::AssetVariant::ReIssuanceAsset { .. } => "re_issuance_asset",
        }
        .into()
    }

    /// Return the asset id when this asset is the `asset_id` variant.
    pub fn asset_id_value(&self) -> Option<AssetId> {
        match self.inner {
            abi::AssetVariant::AssetId { asset_id } => Some(asset_id.into()),
            abi::AssetVariant::NewIssuanceAsset { .. }
            | abi::AssetVariant::NewIssuanceToken { .. }
            | abi::AssetVariant::ReIssuanceAsset { .. } => None,
        }
    }

    /// Return the input index for issuance-derived asset variants.
    pub fn input_index(&self) -> Option<u32> {
        match self.inner {
            abi::AssetVariant::AssetId { .. } => None,
            abi::AssetVariant::NewIssuanceAsset { input_index }
            | abi::AssetVariant::NewIssuanceToken { input_index }
            | abi::AssetVariant::ReIssuanceAsset { input_index } => Some(input_index),
        }
    }
}

/// A Wallet ABI output blinder variant.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiBlinderVariant {
    pub(crate) inner: abi::BlinderVariant,
}

#[uniffi::export]
impl WalletAbiBlinderVariant {
    /// Build the Wallet ABI `wallet` blinder variant.
    #[uniffi::constructor]
    pub fn wallet() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::BlinderVariant::Wallet,
        })
    }

    /// Build the Wallet ABI `provided` blinder variant.
    #[uniffi::constructor]
    pub fn provided(pubkey: &PublicKey) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::BlinderVariant::Provided {
                pubkey: elements::secp256k1_zkp::PublicKey::from_slice(&pubkey.to_bytes())?,
            },
        }))
    }

    /// Build the Wallet ABI `explicit` blinder variant.
    #[uniffi::constructor]
    pub fn explicit() -> Arc<Self> {
        Arc::new(Self {
            inner: abi::BlinderVariant::Explicit,
        })
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::BlinderVariant::Wallet => "wallet",
            abi::BlinderVariant::Provided { .. } => "provided",
            abi::BlinderVariant::Explicit => "explicit",
        }
        .into()
    }

    /// Return the pubkey when this blinder is the `provided` variant.
    pub fn provided_pubkey(&self) -> Option<Arc<PublicKey>> {
        match &self.inner {
            abi::BlinderVariant::Provided { pubkey } => {
                PublicKey::from_bytes(&pubkey.serialize()).ok()
            }
            abi::BlinderVariant::Wallet | abi::BlinderVariant::Explicit => None,
        }
    }
}

/// A Wallet ABI output schema entry.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiOutputSchema {
    pub(crate) inner: abi::OutputSchema,
}

#[uniffi::export]
impl WalletAbiOutputSchema {
    /// Build an output schema entry.
    #[uniffi::constructor]
    pub fn new(
        id: &str,
        amount_sat: u64,
        lock: &WalletAbiLockVariant,
        asset: &WalletAbiAssetVariant,
        blinder: &WalletAbiBlinderVariant,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::OutputSchema {
                id: id.to_string(),
                amount_sat,
                lock: lock.inner.clone(),
                asset: asset.inner.clone(),
                blinder: blinder.inner.clone(),
            },
        })
    }

    /// Return the output identifier.
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Return the output amount in satoshi.
    pub fn amount_sat(&self) -> u64 {
        self.inner.amount_sat
    }

    /// Return the output lock variant.
    pub fn lock(&self) -> Arc<WalletAbiLockVariant> {
        Arc::new(WalletAbiLockVariant {
            inner: self.inner.lock.clone(),
        })
    }

    /// Return the output asset variant.
    pub fn asset(&self) -> Arc<WalletAbiAssetVariant> {
        Arc::new(WalletAbiAssetVariant {
            inner: self.inner.asset.clone(),
        })
    }

    /// Return the output blinder variant.
    pub fn blinder(&self) -> Arc<WalletAbiBlinderVariant> {
        Arc::new(WalletAbiBlinderVariant {
            inner: self.inner.blinder.clone(),
        })
    }
}

/// Runtime parameters for a Wallet ABI transaction creation request.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiRuntimeParams {
    pub(crate) inner: abi::RuntimeParams,
}

#[uniffi::export]
impl WalletAbiRuntimeParams {
    /// Build runtime parameters from inputs, outputs, and optional fee settings.
    #[uniffi::constructor]
    pub fn new(
        inputs: &[Arc<WalletAbiInputSchema>],
        outputs: &[Arc<WalletAbiOutputSchema>],
        fee_rate_sat_kvb: Option<f32>,
        lock_time: Option<Arc<LockTime>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::RuntimeParams {
                inputs: inputs.iter().map(|input| input.inner.clone()).collect(),
                outputs: outputs.iter().map(|output| output.inner.clone()).collect(),
                fee_rate_sat_kvb,
                lock_time: lock_time
                    .as_ref()
                    .map(|lock_time| lock_time.as_ref().into()),
            },
        })
    }

    /// Return the runtime input list.
    pub fn inputs(&self) -> Vec<Arc<WalletAbiInputSchema>> {
        self.inner
            .inputs
            .iter()
            .cloned()
            .map(|inner| Arc::new(WalletAbiInputSchema { inner }))
            .collect()
    }

    /// Return the runtime output list.
    pub fn outputs(&self) -> Vec<Arc<WalletAbiOutputSchema>> {
        self.inner
            .outputs
            .iter()
            .cloned()
            .map(|inner| Arc::new(WalletAbiOutputSchema { inner }))
            .collect()
    }

    /// Return the fee rate in satoshi per kvB when present.
    pub fn fee_rate_sat_kvb(&self) -> Option<f32> {
        self.inner.fee_rate_sat_kvb
    }

    /// Return the lock time when present.
    pub fn lock_time(&self) -> Option<Arc<LockTime>> {
        self.inner
            .lock_time
            .map(|lock_time| Arc::new(lock_time.into()))
    }
}
