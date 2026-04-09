use crate::error::WalletAbiError;

use serde::{Deserialize, Serialize};

use lwk_wollet::elements::{AssetId, Script};

/// High-level preview payload for preflight transaction rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestPreview {
    /// Net wallet balance deltas grouped by asset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_deltas: Vec<PreviewAssetDelta>,
    /// Materialized outputs in transaction order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PreviewOutput>,
    /// Producer-provided warnings for the caller UI.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl RequestPreview {
    /// Serialize this preview for insertion into `artifacts.preview`.
    pub fn to_artifact_value(&self) -> Result<serde_json::Value, WalletAbiError> {
        serde_json::to_value(self).map_err(WalletAbiError::from)
    }

    /// Parse a preview from `artifacts.preview`.
    pub fn from_artifact_value(value: &serde_json::Value) -> Result<Self, WalletAbiError> {
        serde_json::from_value(value.clone()).map_err(|error| {
            WalletAbiError::InvalidResponse(format!("invalid preview artifact payload: {error}"))
        })
    }
}

/// Wallet balance delta preview for one asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewAssetDelta {
    /// Asset identifier for this wallet delta entry.
    pub asset_id: AssetId,
    /// Signed wallet delta in satoshis for this asset.
    pub wallet_delta_sat: i64,
}

/// Materialized output preview entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewOutput {
    /// Output classification for the caller UI.
    pub kind: PreviewOutputKind,
    /// Asset identifier for the output.
    pub asset_id: AssetId,
    /// Output amount in satoshis.
    pub amount_sat: u64,
    /// Output locking script.
    pub script_pubkey: Script,
}

/// High-level output classifications exposed in previews.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreviewOutputKind {
    /// Wallet-owned receive output.
    Receive,
    /// Wallet-owned change output.
    Change,
    /// Non-wallet or contract-directed output.
    External,
    /// Fee output added by runtime.
    Fee,
}

#[cfg(test)]
mod tests {
    use super::{PreviewAssetDelta, PreviewOutput, PreviewOutputKind, RequestPreview};

    #[test]
    fn request_preview_roundtrip() {
        let preview = RequestPreview {
            asset_deltas: vec![PreviewAssetDelta {
                asset_id: lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset(),
                wallet_delta_sat: -1_500,
            }],
            outputs: vec![PreviewOutput {
                kind: PreviewOutputKind::External,
                asset_id: lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset(),
                amount_sat: 1_500,
                script_pubkey: lwk_wollet::elements::Script::new(),
            }],
            warnings: vec!["requires confirmation".to_string()],
        };

        let value = preview.to_artifact_value().expect("serialize preview");
        let decoded = RequestPreview::from_artifact_value(&value).expect("deserialize preview");

        assert_eq!(decoded, preview);
    }
}
