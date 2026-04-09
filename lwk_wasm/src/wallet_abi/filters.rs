use crate::{AssetId, Error};

use std::str::FromStr;

use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;
use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A canonical external taproot handle used by Wallet ABI callers.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiTaprootHandle {
    inner: TaprootPubkeyGen,
}

impl std::fmt::Display for WalletAbiTaprootHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<TaprootPubkeyGen> for WalletAbiTaprootHandle {
    fn from(inner: TaprootPubkeyGen) -> Self {
        Self { inner }
    }
}

impl From<WalletAbiTaprootHandle> for TaprootPubkeyGen {
    fn from(value: WalletAbiTaprootHandle) -> Self {
        value.inner
    }
}

impl From<&WalletAbiTaprootHandle> for TaprootPubkeyGen {
    fn from(value: &WalletAbiTaprootHandle) -> Self {
        value.inner.clone()
    }
}

/// An asset selector for wallet-funded inputs.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiAssetFilter {
    inner: abi::AssetFilter,
}

#[wasm_bindgen]
impl WalletAbiAssetFilter {
    /// Build the Wallet ABI `none` asset filter variant.
    pub fn none() -> WalletAbiAssetFilter {
        Self {
            inner: abi::AssetFilter::None,
        }
    }

    /// Build the Wallet ABI `exact` asset filter variant.
    pub fn exact(asset_id: &AssetId) -> WalletAbiAssetFilter {
        Self {
            inner: abi::AssetFilter::Exact {
                asset_id: (*asset_id).into(),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AssetFilter::None => "none",
            abi::AssetFilter::Exact { .. } => "exact",
        }
        .to_string()
    }

    /// Return the asset id when this filter is the `exact` variant.
    #[wasm_bindgen(js_name = exactAssetId)]
    pub fn exact_asset_id(&self) -> Option<AssetId> {
        match self.inner {
            abi::AssetFilter::Exact { asset_id } => Some(asset_id.into()),
            abi::AssetFilter::None => None,
        }
    }
}

/// An amount selector for wallet-funded inputs.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiAmountFilter {
    inner: abi::AmountFilter,
}

#[wasm_bindgen]
impl WalletAbiAmountFilter {
    /// Build the Wallet ABI `none` amount filter variant.
    pub fn none() -> WalletAbiAmountFilter {
        Self {
            inner: abi::AmountFilter::None,
        }
    }

    /// Build the Wallet ABI `exact` amount filter variant.
    pub fn exact(amount_sat: u64) -> WalletAbiAmountFilter {
        Self {
            inner: abi::AmountFilter::Exact { amount_sat },
        }
    }

    /// Build the Wallet ABI `min` amount filter variant.
    pub fn min(amount_sat: u64) -> WalletAbiAmountFilter {
        Self {
            inner: abi::AmountFilter::Min { amount_sat },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::AmountFilter::None => "none",
            abi::AmountFilter::Exact { .. } => "exact",
            abi::AmountFilter::Min { .. } => "min",
        }
        .to_string()
    }

    /// Return the selected amount for `exact` and `min` variants.
    #[wasm_bindgen(js_name = amountSat)]
    pub fn amount_sat(&self) -> Option<u64> {
        match self.inner {
            abi::AmountFilter::Exact { amount_sat } | abi::AmountFilter::Min { amount_sat } => {
                Some(amount_sat)
            }
            abi::AmountFilter::None => None,
        }
    }
}

#[wasm_bindgen]
impl WalletAbiTaprootHandle {
    /// Parse the canonical `<seed_or_ext-xonly_hex>:<pubkey>:<address>` taproot-handle string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<WalletAbiTaprootHandle, Error> {
        TaprootPubkeyGen::from_str(s)
            .map(Into::into)
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Return the canonical taproot-handle string.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiTaprootHandle};

    use std::str::FromStr;

    use lwk_common::Network;
    use lwk_simplicity::simplicityhl::elements::Address;
    use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;

    use crate::Network as WasmNetwork;

    #[test]
    fn wallet_abi_taproot_handle_roundtrip() {
        let handle = TaprootPubkeyGen::from(
            &(),
            Network::Liquid,
            &|_, _, _| {
                Ok(Address::from_str("lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv")
                    .expect("valid fixed address"))
            },
        )
        .expect("build taproot handle")
        .to_string();
        let parsed = WalletAbiTaprootHandle::from_string(&handle).expect("parse handle");

        assert_eq!(parsed.to_string(), handle);
    }

    #[test]
    fn wallet_abi_asset_filter_roundtrip() {
        let policy_asset = WasmNetwork::testnet().policy_asset();
        let filter = WalletAbiAssetFilter::exact(&policy_asset);

        assert_eq!(filter.kind(), "exact");
        assert_eq!(filter.exact_asset_id(), Some(policy_asset));
        assert_eq!(WalletAbiAssetFilter::none().kind(), "none");
        assert_eq!(WalletAbiAssetFilter::none().exact_asset_id(), None);
    }

    #[test]
    fn wallet_abi_amount_filter_roundtrip() {
        let exact = WalletAbiAmountFilter::exact(1_500);
        let minimum = WalletAbiAmountFilter::min(600);
        let none = WalletAbiAmountFilter::none();

        assert_eq!(exact.kind(), "exact");
        assert_eq!(exact.amount_sat(), Some(1_500));
        assert_eq!(minimum.kind(), "min");
        assert_eq!(minimum.amount_sat(), Some(600));
        assert_eq!(none.kind(), "none");
        assert_eq!(none.amount_sat(), None);
    }
}
