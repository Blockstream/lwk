use std::sync::Arc;

use crate::blockdata::script::Script;
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

/// High-level output classifications exposed in previews.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
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

/// Materialized output preview entry.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiPreviewOutput {
    pub(crate) inner: abi::PreviewOutput,
}

#[uniffi::export]
impl WalletAbiPreviewOutput {
    /// Build a preview output entry.
    #[uniffi::constructor]
    pub fn new(
        kind: WalletAbiPreviewOutputKind,
        asset_id: AssetId,
        amount_sat: u64,
        script_pubkey: &Script,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::PreviewOutput {
                kind: kind.into(),
                asset_id: asset_id.into(),
                amount_sat,
                script_pubkey: script_pubkey.into(),
            },
        })
    }

    /// Parse canonical Wallet ABI preview output JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this preview output to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the output classification.
    pub fn kind(&self) -> WalletAbiPreviewOutputKind {
        self.inner.kind.into()
    }

    /// Return the asset identifier for this output.
    pub fn asset_id(&self) -> AssetId {
        self.inner.asset_id.into()
    }

    /// Return the output amount in satoshis.
    pub fn amount_sat(&self) -> u64 {
        self.inner.amount_sat
    }

    /// Return the output locking script.
    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey.clone().into())
    }
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

/// High-level preview payload for preflight transaction rendering.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiRequestPreview {
    pub(crate) inner: abi::RequestPreview,
}

#[uniffi::export]
impl WalletAbiRequestPreview {
    /// Build a request preview payload.
    #[uniffi::constructor]
    pub fn new(
        asset_deltas: Vec<Arc<WalletAbiPreviewAssetDelta>>,
        outputs: Vec<Arc<WalletAbiPreviewOutput>>,
        warnings: Vec<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::RequestPreview {
                asset_deltas: asset_deltas
                    .into_iter()
                    .map(|delta| delta.inner.clone())
                    .collect(),
                outputs: outputs
                    .into_iter()
                    .map(|output| output.inner.clone())
                    .collect(),
                warnings,
            },
        })
    }

    /// Parse canonical Wallet ABI request preview JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this request preview to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the wallet asset delta entries.
    pub fn asset_deltas(&self) -> Vec<Arc<WalletAbiPreviewAssetDelta>> {
        self.inner
            .asset_deltas
            .iter()
            .cloned()
            .map(|inner| Arc::new(WalletAbiPreviewAssetDelta { inner }))
            .collect()
    }

    /// Return the materialized outputs in transaction order.
    pub fn outputs(&self) -> Vec<Arc<WalletAbiPreviewOutput>> {
        self.inner
            .outputs
            .iter()
            .cloned()
            .map(|inner| Arc::new(WalletAbiPreviewOutput { inner }))
            .collect()
    }

    /// Return producer-provided warnings for the caller UI.
    pub fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput, WalletAbiPreviewOutputKind,
        WalletAbiRequestPreview,
    };
    use crate::blockdata::script::Script;
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

    #[test]
    fn wallet_abi_preview_output_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let script = Script::empty();
        let output = WalletAbiPreviewOutput::new(
            WalletAbiPreviewOutputKind::External,
            asset_id,
            1_500,
            &script,
        );

        let json = output.to_json().expect("serialize preview output");
        let decoded = WalletAbiPreviewOutput::from_json(&json).expect("deserialize preview output");

        assert_eq!(decoded.kind(), WalletAbiPreviewOutputKind::External);
        assert_eq!(decoded.asset_id(), asset_id);
        assert_eq!(decoded.amount_sat(), 1_500);
        assert_eq!(decoded.script_pubkey().to_string(), "");
    }

    #[test]
    fn wallet_abi_request_preview_roundtrip() {
        let asset_id = Network::testnet().policy_asset();
        let preview = WalletAbiRequestPreview::new(
            vec![WalletAbiPreviewAssetDelta::new(asset_id, -1_500)],
            vec![WalletAbiPreviewOutput::new(
                WalletAbiPreviewOutputKind::Fee,
                asset_id,
                600,
                &Script::empty(),
            )],
            vec!["requires confirmation".to_string()],
        );

        let json = preview.to_json().expect("serialize request preview");
        let decoded = WalletAbiRequestPreview::from_json(&json).expect("deserialize request preview");

        assert_eq!(decoded.asset_deltas()[0].wallet_delta_sat(), -1_500);
        assert_eq!(decoded.outputs()[0].kind(), WalletAbiPreviewOutputKind::Fee);
        assert_eq!(
            decoded.warnings(),
            vec!["requires confirmation".to_string()]
        );
    }
}
