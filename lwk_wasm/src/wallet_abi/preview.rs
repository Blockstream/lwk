use crate::{AssetId, Script};

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

/// High-level output classifications exposed in previews.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiPreviewOutputKind {
    /// Wallet-owned receive output.
    Receive,
    /// Wallet-owned change output.
    Change,
    /// Non-wallet or contract-directed output.
    External,
    /// Fee output added by runtime.
    Fee,
}

impl From<abi::PreviewOutputKind> for WalletAbiPreviewOutputKind {
    fn from(value: abi::PreviewOutputKind) -> Self {
        match value {
            abi::PreviewOutputKind::Receive => Self::Receive,
            abi::PreviewOutputKind::Change => Self::Change,
            abi::PreviewOutputKind::External => Self::External,
            abi::PreviewOutputKind::Fee => Self::Fee,
        }
    }
}

impl From<WalletAbiPreviewOutputKind> for abi::PreviewOutputKind {
    fn from(value: WalletAbiPreviewOutputKind) -> Self {
        match value {
            WalletAbiPreviewOutputKind::Receive => Self::Receive,
            WalletAbiPreviewOutputKind::Change => Self::Change,
            WalletAbiPreviewOutputKind::External => Self::External,
            WalletAbiPreviewOutputKind::Fee => Self::Fee,
        }
    }
}

/// Materialized output preview entry.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiPreviewOutput {
    inner: abi::PreviewOutput,
}

#[wasm_bindgen]
impl WalletAbiPreviewOutput {
    /// Build a preview output entry.
    pub fn new(
        kind: WalletAbiPreviewOutputKind,
        asset_id: &AssetId,
        amount_sat: u64,
        script_pubkey: &Script,
    ) -> WalletAbiPreviewOutput {
        Self {
            inner: abi::PreviewOutput {
                kind: kind.into(),
                asset_id: (*asset_id).into(),
                amount_sat,
                script_pubkey: script_pubkey.as_ref().clone(),
            },
        }
    }

    /// Return the output classification.
    pub fn kind(&self) -> WalletAbiPreviewOutputKind {
        self.inner.kind.into()
    }

    /// Return the asset identifier for this output.
    #[wasm_bindgen(js_name = assetId)]
    pub fn asset_id(&self) -> AssetId {
        self.inner.asset_id.into()
    }

    /// Return the output amount in satoshis.
    #[wasm_bindgen(js_name = amountSat)]
    pub fn amount_sat(&self) -> u64 {
        self.inner.amount_sat
    }

    /// Return the output locking script.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey.clone().into()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput, WalletAbiPreviewOutputKind,
    };

    use crate::{Network, Script};

    #[test]
    fn wallet_abi_preview_delta_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let delta = WalletAbiPreviewAssetDelta::new(&asset_id, -1_500);

        assert_eq!(delta.asset_id(), asset_id);
        assert_eq!(delta.wallet_delta_sat(), -1_500);
    }

    #[test]
    fn wallet_abi_preview_output_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let script = Script::new("6a").expect("script");
        let output = WalletAbiPreviewOutput::new(
            WalletAbiPreviewOutputKind::Fee,
            &asset_id,
            600,
            &script,
        );

        assert_eq!(output.kind(), WalletAbiPreviewOutputKind::Fee);
        assert_eq!(output.asset_id(), asset_id);
        assert_eq!(output.amount_sat(), 600);
        assert_eq!(output.script_pubkey().to_string(), script.to_string());
    }
}
