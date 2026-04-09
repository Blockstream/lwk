use std::sync::Arc;

use crate::types::AssetId;
use crate::LwkError;
use crate::wallet_abi::abi;

/// Wallet balance delta preview for one asset.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiPreviewAssetDelta {
    pub(crate) inner: abi::PreviewAssetDelta,
}

#[uniffi::export]
impl WalletAbiPreviewAssetDelta {
    /// Build a wallet preview delta from an asset identifier and signed amount.
    #[uniffi::constructor]
    pub fn new(asset_id: AssetId, wallet_delta_sat: i64) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::PreviewAssetDelta {
                asset_id: asset_id.into(),
                wallet_delta_sat,
            },
        })
    }

    /// Parse canonical Wallet ABI preview delta JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this preview delta to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the asset identifier for this delta entry.
    pub fn asset_id(&self) -> AssetId {
        self.inner.asset_id.into()
    }

    /// Return the signed wallet delta in satoshis.
    pub fn wallet_delta_sat(&self) -> i64 {
        self.inner.wallet_delta_sat
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiPreviewAssetDelta;
    use crate::Network;

    #[test]
    fn wallet_abi_preview_asset_delta_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let delta = WalletAbiPreviewAssetDelta::new(asset_id, -1_500);

        let json = delta.to_json().expect("serialize preview delta");
        let decoded =
            WalletAbiPreviewAssetDelta::from_json(&json).expect("deserialize preview delta");

        assert_eq!(decoded.asset_id(), asset_id);
        assert_eq!(decoded.wallet_delta_sat(), -1_500);
    }
}
