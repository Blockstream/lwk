use super::filters::WalletAbiFinalizerSpec;

use crate::{AssetId, Script};

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A Wallet ABI output lock variant.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiLockVariant {
    inner: abi::LockVariant,
}

#[wasm_bindgen]
impl WalletAbiLockVariant {
    /// Build the Wallet ABI `wallet` lock variant.
    pub fn wallet() -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Wallet,
        }
    }

    /// Build the Wallet ABI `script` lock variant.
    pub fn script(script: &Script) -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Script {
                script: script.as_ref().clone(),
            },
        }
    }

    /// Build the Wallet ABI `finalizer` lock variant.
    pub fn finalizer(finalizer: &WalletAbiFinalizerSpec) -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Finalizer {
                finalizer: Box::new(finalizer.clone().inner),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::LockVariant::Wallet => "wallet",
            abi::LockVariant::Script { .. } => "script",
            abi::LockVariant::Finalizer { .. } => "finalizer",
        }
        .to_string()
    }

    /// Return the script when this lock is the `script` variant.
    #[wasm_bindgen(js_name = scriptValue)]
    pub fn script_value(&self) -> Option<Script> {
        match &self.inner {
            abi::LockVariant::Script { script } => Some(script.clone().into()),
            abi::LockVariant::Wallet | abi::LockVariant::Finalizer { .. } => None,
        }
    }

    /// Return the finalizer when this lock is the `finalizer` variant.
    #[wasm_bindgen(js_name = finalizerValue)]
    pub fn finalizer_value(&self) -> Option<WalletAbiFinalizerSpec> {
        match &self.inner {
            abi::LockVariant::Finalizer { finalizer } => Some(WalletAbiFinalizerSpec {
                inner: (**finalizer).clone(),
            }),
            abi::LockVariant::Wallet | abi::LockVariant::Script { .. } => None,
        }
    }
}

/// A Wallet ABI output asset variant.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiAssetVariant {
    inner: abi::AssetVariant,
}

#[wasm_bindgen]
impl WalletAbiAssetVariant {
    /// Build the Wallet ABI `asset_id` asset variant.
    #[wasm_bindgen(js_name = assetId)]
    pub fn asset_id(asset_id: &AssetId) -> WalletAbiAssetVariant {
        Self {
            inner: abi::AssetVariant::AssetId {
                asset_id: (*asset_id).into(),
            },
        }
    }

    /// Build the Wallet ABI `new_issuance_asset` asset variant.
    #[wasm_bindgen(js_name = newIssuanceAsset)]
    pub fn new_issuance_asset(input_index: u32) -> WalletAbiAssetVariant {
        Self {
            inner: abi::AssetVariant::NewIssuanceAsset { input_index },
        }
    }

    /// Build the Wallet ABI `new_issuance_token` asset variant.
    #[wasm_bindgen(js_name = newIssuanceToken)]
    pub fn new_issuance_token(input_index: u32) -> WalletAbiAssetVariant {
        Self {
            inner: abi::AssetVariant::NewIssuanceToken { input_index },
        }
    }

    /// Build the Wallet ABI `re_issuance_asset` asset variant.
    #[wasm_bindgen(js_name = reIssuanceAsset)]
    pub fn re_issuance_asset(input_index: u32) -> WalletAbiAssetVariant {
        Self {
            inner: abi::AssetVariant::ReIssuanceAsset { input_index },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AssetVariant::AssetId { .. } => "asset_id",
            abi::AssetVariant::NewIssuanceAsset { .. } => "new_issuance_asset",
            abi::AssetVariant::NewIssuanceToken { .. } => "new_issuance_token",
            abi::AssetVariant::ReIssuanceAsset { .. } => "re_issuance_asset",
        }
        .to_string()
    }

    /// Return the asset id when this asset is the `asset_id` variant.
    #[wasm_bindgen(js_name = assetIdValue)]
    pub fn asset_id_value(&self) -> Option<AssetId> {
        match self.inner {
            abi::AssetVariant::AssetId { asset_id } => Some(asset_id.into()),
            abi::AssetVariant::NewIssuanceAsset { .. }
            | abi::AssetVariant::NewIssuanceToken { .. }
            | abi::AssetVariant::ReIssuanceAsset { .. } => None,
        }
    }

    /// Return the input index for issuance-derived asset variants.
    #[wasm_bindgen(js_name = inputIndex)]
    pub fn input_index(&self) -> Option<u32> {
        match self.inner {
            abi::AssetVariant::AssetId { .. } => None,
            abi::AssetVariant::NewIssuanceAsset { input_index }
            | abi::AssetVariant::NewIssuanceToken { input_index }
            | abi::AssetVariant::ReIssuanceAsset { input_index } => Some(input_index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiAssetVariant, WalletAbiLockVariant};

    use crate::{Network, Script, WalletAbiFinalizerSpec};

    #[test]
    fn wallet_abi_lock_variant_roundtrip() {
        let script = Script::new("6a").expect("script");
        let script_variant = WalletAbiLockVariant::script(&script);
        let finalizer_variant = WalletAbiLockVariant::finalizer(&WalletAbiFinalizerSpec::wallet());

        assert_eq!(WalletAbiLockVariant::wallet().kind(), "wallet");
        assert_eq!(script_variant.kind(), "script");
        assert_eq!(
            script_variant.script_value().expect("script value").to_string(),
            script.to_string()
        );
        assert!(script_variant.finalizer_value().is_none());
        assert_eq!(finalizer_variant.kind(), "finalizer");
        assert_eq!(
            finalizer_variant
                .finalizer_value()
                .expect("finalizer value")
                .kind(),
            "wallet"
        );
    }

    #[test]
    fn wallet_abi_asset_variant_roundtrip() {
        let policy_asset = Network::testnet().policy_asset();
        let asset_id_variant = WalletAbiAssetVariant::asset_id(&policy_asset);
        let issuance_variant = WalletAbiAssetVariant::new_issuance_asset(2);
        let token_variant = WalletAbiAssetVariant::new_issuance_token(3);
        let reissuance_variant = WalletAbiAssetVariant::re_issuance_asset(4);

        assert_eq!(asset_id_variant.kind(), "asset_id");
        assert_eq!(asset_id_variant.asset_id_value(), Some(policy_asset));
        assert_eq!(asset_id_variant.input_index(), None);

        assert_eq!(issuance_variant.kind(), "new_issuance_asset");
        assert_eq!(issuance_variant.asset_id_value(), None);
        assert_eq!(issuance_variant.input_index(), Some(2));
        assert_eq!(token_variant.kind(), "new_issuance_token");
        assert_eq!(token_variant.input_index(), Some(3));
        assert_eq!(reissuance_variant.kind(), "re_issuance_asset");
        assert_eq!(reissuance_variant.input_index(), Some(4));
    }
}
