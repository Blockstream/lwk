use crate::AssetId;

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// Wallet balance delta preview for one asset.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiPreviewAssetDelta {
    inner: abi::PreviewAssetDelta,
}

#[wasm_bindgen]
impl WalletAbiPreviewAssetDelta {
    /// Build a wallet preview delta from an asset identifier and signed amount.
    pub fn new(asset_id: &AssetId, wallet_delta_sat: i64) -> WalletAbiPreviewAssetDelta {
        Self {
            inner: abi::PreviewAssetDelta {
                asset_id: (*asset_id).into(),
                wallet_delta_sat,
            },
        }
    }

    /// Return the asset identifier for this delta entry.
    #[wasm_bindgen(js_name = assetId)]
    pub fn asset_id(&self) -> AssetId {
        self.inner.asset_id.into()
    }

    /// Return the signed wallet delta in satoshis.
    #[wasm_bindgen(js_name = walletDeltaSat)]
    pub fn wallet_delta_sat(&self) -> i64 {
        self.inner.wallet_delta_sat
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiPreviewAssetDelta;

    use crate::Network;

    #[test]
    fn wallet_abi_preview_delta_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let delta = WalletAbiPreviewAssetDelta::new(&asset_id, -1_500);

        assert_eq!(delta.asset_id(), asset_id);
        assert_eq!(delta.wallet_delta_sat(), -1_500);
    }
}
