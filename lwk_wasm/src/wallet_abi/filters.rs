use crate::{AssetId, Error, OutPoint, Script};

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

/// A lock selector for wallet-funded inputs.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiLockFilter {
    inner: abi::LockFilter,
}

#[wasm_bindgen]
impl WalletAbiLockFilter {
    /// Build the Wallet ABI `none` lock filter variant.
    pub fn none() -> WalletAbiLockFilter {
        Self {
            inner: abi::LockFilter::None,
        }
    }

    /// Build the Wallet ABI `script` lock filter variant.
    pub fn script(script: &Script) -> WalletAbiLockFilter {
        Self {
            inner: abi::LockFilter::Script {
                script: script.as_ref().clone(),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::LockFilter::None => "none",
            abi::LockFilter::Script { .. } => "script",
        }
        .to_string()
    }

    /// Return the script when this filter is the `script` variant.
    #[wasm_bindgen(js_name = scriptValue)]
    pub fn script_value(&self) -> Option<Script> {
        match &self.inner {
            abi::LockFilter::Script { script } => Some(script.clone().into()),
            abi::LockFilter::None => None,
        }
    }
}

/// A grouped filter for selecting wallet-funded UTXOs.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletSourceFilter {
    inner: abi::WalletSourceFilter,
}

#[wasm_bindgen]
impl WalletAbiWalletSourceFilter {
    /// Build a wallet source filter from asset, amount, and lock filters.
    #[wasm_bindgen(js_name = withFilters)]
    pub fn with_filters(
        asset: &WalletAbiAssetFilter,
        amount: &WalletAbiAmountFilter,
        lock: &WalletAbiLockFilter,
    ) -> WalletAbiWalletSourceFilter {
        Self {
            inner: abi::WalletSourceFilter {
                asset: asset.inner.clone(),
                amount: amount.inner.clone(),
                lock: lock.inner.clone(),
            },
        }
    }

    /// Build a wallet source filter that matches any wallet UTXO.
    pub fn any() -> WalletAbiWalletSourceFilter {
        Self {
            inner: abi::WalletSourceFilter::default(),
        }
    }

    /// Return the asset filter component.
    pub fn asset(&self) -> WalletAbiAssetFilter {
        WalletAbiAssetFilter {
            inner: self.inner.asset.clone(),
        }
    }

    /// Return the amount filter component.
    pub fn amount(&self) -> WalletAbiAmountFilter {
        WalletAbiAmountFilter {
            inner: self.inner.amount.clone(),
        }
    }

    /// Return the lock filter component.
    pub fn lock(&self) -> WalletAbiLockFilter {
        WalletAbiLockFilter {
            inner: self.inner.lock.clone(),
        }
    }
}

/// A Wallet ABI input UTXO source.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiUtxoSource {
    inner: abi::UTXOSource,
}

#[wasm_bindgen]
impl WalletAbiUtxoSource {
    /// Build the Wallet ABI `wallet` UTXO source variant.
    pub fn wallet(filter: &WalletAbiWalletSourceFilter) -> WalletAbiUtxoSource {
        Self {
            inner: abi::UTXOSource::Wallet {
                filter: filter.inner.clone(),
            },
        }
    }

    /// Build the Wallet ABI `provided` UTXO source variant.
    pub fn provided(outpoint: &OutPoint) -> WalletAbiUtxoSource {
        Self {
            inner: abi::UTXOSource::Provided {
                outpoint: outpoint.into(),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::UTXOSource::Wallet { .. } => "wallet",
            abi::UTXOSource::Provided { .. } => "provided",
        }
        .to_string()
    }

    /// Return the wallet filter when this source is the `wallet` variant.
    #[wasm_bindgen(js_name = walletFilter)]
    pub fn wallet_filter(&self) -> Option<WalletAbiWalletSourceFilter> {
        match &self.inner {
            abi::UTXOSource::Wallet { filter } => Some(WalletAbiWalletSourceFilter {
                inner: filter.clone(),
            }),
            abi::UTXOSource::Provided { .. } => None,
        }
    }

    /// Return the outpoint when this source is the `provided` variant.
    #[wasm_bindgen(js_name = providedOutpoint)]
    pub fn provided_outpoint(&self) -> Option<OutPoint> {
        match self.inner {
            abi::UTXOSource::Provided { outpoint } => Some(outpoint.into()),
            abi::UTXOSource::Wallet { .. } => None,
        }
    }
}

/// The issuance kind for a Wallet ABI input issuance.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiInputIssuanceKind {
    /// Create a new asset issuance.
    New,
    /// Reissue an existing asset.
    Reissue,
}

impl From<abi::InputIssuanceKind> for WalletAbiInputIssuanceKind {
    fn from(value: abi::InputIssuanceKind) -> Self {
        match value {
            abi::InputIssuanceKind::New => Self::New,
            abi::InputIssuanceKind::Reissue => Self::Reissue,
        }
    }
}

impl From<WalletAbiInputIssuanceKind> for abi::InputIssuanceKind {
    fn from(value: WalletAbiInputIssuanceKind) -> Self {
        match value {
            WalletAbiInputIssuanceKind::New => Self::New,
            WalletAbiInputIssuanceKind::Reissue => Self::Reissue,
        }
    }
}

/// Wallet ABI issuance data attached to an input.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiInputIssuance {
    inner: abi::InputIssuance,
}

#[wasm_bindgen]
impl WalletAbiInputIssuance {
    /// Build a `new` input issuance object.
    pub fn new(
        asset_amount_sat: u64,
        token_amount_sat: u64,
        entropy: &[u8],
    ) -> Result<WalletAbiInputIssuance, Error> {
        Ok(Self {
            inner: abi::InputIssuance {
                kind: abi::InputIssuanceKind::New,
                asset_amount_sat,
                token_amount_sat,
                entropy: entropy.try_into()?,
            },
        })
    }

    /// Build a `reissue` input issuance object.
    pub fn reissue(
        asset_amount_sat: u64,
        token_amount_sat: u64,
        entropy: &[u8],
    ) -> Result<WalletAbiInputIssuance, Error> {
        Ok(Self {
            inner: abi::InputIssuance {
                kind: abi::InputIssuanceKind::Reissue,
                asset_amount_sat,
                token_amount_sat,
                entropy: entropy.try_into()?,
            },
        })
    }

    /// Return the issuance kind.
    pub fn kind(&self) -> WalletAbiInputIssuanceKind {
        self.inner.kind.clone().into()
    }

    /// Return the issued asset amount in satoshi.
    #[wasm_bindgen(js_name = assetAmountSat)]
    pub fn asset_amount_sat(&self) -> u64 {
        self.inner.asset_amount_sat
    }

    /// Return the issued token amount in satoshi.
    #[wasm_bindgen(js_name = tokenAmountSat)]
    pub fn token_amount_sat(&self) -> u64 {
        self.inner.token_amount_sat
    }

    /// Return the issuance entropy bytes.
    pub fn entropy(&self) -> Vec<u8> {
        self.inner.entropy.to_vec()
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
    use super::{
        WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiLockFilter, WalletAbiTaprootHandle,
        WalletAbiInputIssuance, WalletAbiInputIssuanceKind, WalletAbiUtxoSource,
        WalletAbiWalletSourceFilter,
    };

    use std::str::FromStr;

    use lwk_common::Network;
    use lwk_simplicity::simplicityhl::elements::Address;
    use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;

    use crate::{Network as WasmNetwork, OutPoint as WasmOutPoint, Script as WasmScript};

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

    #[test]
    fn wallet_abi_lock_filter_roundtrip() {
        let script = WasmScript::new("6a").expect("op return");
        let filter = WalletAbiLockFilter::script(&script);

        assert_eq!(filter.kind(), "script");
        assert_eq!(
            filter.script_value().expect("script filter").to_string(),
            script.to_string()
        );
        assert_eq!(WalletAbiLockFilter::none().kind(), "none");
        assert!(WalletAbiLockFilter::none().script_value().is_none());
    }

    #[test]
    fn wallet_abi_wallet_source_filter_roundtrip() {
        let policy_asset = WasmNetwork::testnet().policy_asset();
        let filter = WalletAbiWalletSourceFilter::with_filters(
            &WalletAbiAssetFilter::exact(&policy_asset),
            &WalletAbiAmountFilter::min(2_000),
            &WalletAbiLockFilter::script(&WasmScript::new("6a").expect("op return")),
        );

        assert_eq!(filter.asset().exact_asset_id(), Some(policy_asset));
        assert_eq!(filter.amount().amount_sat(), Some(2_000));
        assert_eq!(filter.lock().kind(), "script");
        assert_eq!(WalletAbiWalletSourceFilter::any().asset().kind(), "none");
        assert_eq!(WalletAbiWalletSourceFilter::any().amount().kind(), "none");
        assert_eq!(WalletAbiWalletSourceFilter::any().lock().kind(), "none");
    }

    #[test]
    fn wallet_abi_utxo_source_roundtrip() {
        let wallet_source = WalletAbiUtxoSource::wallet(&WalletAbiWalletSourceFilter::any());
        let provided_outpoint = WasmOutPoint::new(
            "[elements]0000000000000000000000000000000000000000000000000000000000000001:1",
        )
        .expect("outpoint");
        let provided_source = WalletAbiUtxoSource::provided(&provided_outpoint);

        assert_eq!(wallet_source.kind(), "wallet");
        assert_eq!(wallet_source.wallet_filter().expect("wallet filter").lock().kind(), "none");
        assert!(wallet_source.provided_outpoint().is_none());

        assert_eq!(provided_source.kind(), "provided");
        assert!(provided_source.wallet_filter().is_none());
        assert_eq!(
            provided_source
                .provided_outpoint()
                .expect("provided outpoint")
                .to_string(),
            provided_outpoint.to_string()
        );
    }

    #[test]
    fn wallet_abi_input_issuance_roundtrip() {
        let entropy = [7_u8; 32];
        let issuance = WalletAbiInputIssuance::new(1_000, 500, &entropy).expect("issuance");
        let reissuance =
            WalletAbiInputIssuance::reissue(2_000, 250, &entropy).expect("reissuance");

        assert_eq!(issuance.kind(), WalletAbiInputIssuanceKind::New);
        assert_eq!(issuance.asset_amount_sat(), 1_000);
        assert_eq!(issuance.token_amount_sat(), 500);
        assert_eq!(issuance.entropy(), entropy);

        assert_eq!(reissuance.kind(), WalletAbiInputIssuanceKind::Reissue);
        assert_eq!(reissuance.asset_amount_sat(), 2_000);
        assert_eq!(reissuance.token_amount_sat(), 250);
        assert_eq!(reissuance.entropy(), entropy);
    }
}
