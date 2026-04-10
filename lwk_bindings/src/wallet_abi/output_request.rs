use crate::types::AssetId;

/// Wallet-owned output role requested by the runtime.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiWalletOutputRole {
    /// Default receive-style wallet output.
    Receive,
    /// Runtime-added change output.
    Change,
}

/// Deterministic wallet output selector record passed across the foreign callback boundary.
#[derive(uniffi::Record, Clone)]
pub struct WalletAbiWalletOutputRequest {
    /// Requested wallet output role.
    pub role: WalletAbiWalletOutputRole,
    /// Deterministic zero-based ordinal within this role/build pass.
    pub ordinal: u32,
    /// Optional residual asset id for runtime-added change outputs.
    pub asset_id: Option<AssetId>,
}

impl From<&lwk_simplicity::wallet_abi::WalletOutputRequest> for WalletAbiWalletOutputRequest {
    fn from(value: &lwk_simplicity::wallet_abi::WalletOutputRequest) -> Self {
        match value {
            lwk_simplicity::wallet_abi::WalletOutputRequest::Receive { index } => Self {
                role: WalletAbiWalletOutputRole::Receive,
                ordinal: *index,
                asset_id: None,
            },
            lwk_simplicity::wallet_abi::WalletOutputRequest::Change { index, asset_id } => Self {
                role: WalletAbiWalletOutputRole::Change,
                ordinal: *index,
                asset_id: Some((*asset_id).into()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiWalletOutputRequest, WalletAbiWalletOutputRole};
    use crate::Network;
    use lwk_simplicity::wallet_abi::WalletOutputRequest;

    #[test]
    fn wallet_abi_wallet_output_request_maps_runtime_roles_losslessly() {
        let receive =
            WalletAbiWalletOutputRequest::from(&WalletOutputRequest::Receive { index: 3 });
        let change = WalletAbiWalletOutputRequest::from(&WalletOutputRequest::Change {
            index: 5,
            asset_id: Network::testnet().policy_asset().into(),
        });

        assert_eq!(receive.role, WalletAbiWalletOutputRole::Receive);
        assert_eq!(receive.ordinal, 3);
        assert_eq!(receive.asset_id, None);

        assert_eq!(change.role, WalletAbiWalletOutputRole::Change);
        assert_eq!(change.ordinal, 5);
        assert_eq!(change.asset_id, Some(Network::testnet().policy_asset()));
    }
}
