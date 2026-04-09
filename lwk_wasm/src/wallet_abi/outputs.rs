use super::filters::WalletAbiFinalizerSpec;
use super::json_from_str;
use super::json_from_js_value;
use super::js_value_from_json;

use crate::{AssetId, LockTime, PublicKey, Script, WalletAbiInputSchema};

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

/// A Wallet ABI output blinder variant.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiBlinderVariant {
    inner: abi::BlinderVariant,
}

#[wasm_bindgen]
impl WalletAbiBlinderVariant {
    /// Build the Wallet ABI `wallet` blinder variant.
    pub fn wallet() -> WalletAbiBlinderVariant {
        Self {
            inner: abi::BlinderVariant::Wallet,
        }
    }

    /// Build the Wallet ABI `provided` blinder variant.
    pub fn provided(pubkey: &PublicKey) -> Result<WalletAbiBlinderVariant, crate::Error> {
        Ok(Self {
            inner: abi::BlinderVariant::Provided {
                pubkey: lwk_wollet::elements::secp256k1_zkp::PublicKey::from_slice(
                    &pubkey.to_bytes(),
                )?,
            },
        })
    }

    /// Build the Wallet ABI `explicit` blinder variant.
    pub fn explicit() -> WalletAbiBlinderVariant {
        Self {
            inner: abi::BlinderVariant::Explicit,
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::BlinderVariant::Wallet => "wallet",
            abi::BlinderVariant::Provided { .. } => "provided",
            abi::BlinderVariant::Explicit => "explicit",
        }
        .to_string()
    }

    /// Return the pubkey when this blinder is the `provided` variant.
    #[wasm_bindgen(js_name = providedPubkey)]
    pub fn provided_pubkey(&self) -> Option<PublicKey> {
        match &self.inner {
            abi::BlinderVariant::Provided { pubkey } => {
                PublicKey::from_bytes(&pubkey.serialize()).ok()
            }
            abi::BlinderVariant::Wallet | abi::BlinderVariant::Explicit => None,
        }
    }
}

/// A Wallet ABI output schema entry.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiOutputSchema {
    pub(crate) inner: abi::OutputSchema,
}

#[wasm_bindgen]
impl WalletAbiOutputSchema {
    /// Build an output schema entry.
    pub fn new(
        id: &str,
        amount_sat: u64,
        lock: &WalletAbiLockVariant,
        asset: &WalletAbiAssetVariant,
        blinder: &WalletAbiBlinderVariant,
    ) -> WalletAbiOutputSchema {
        Self {
            inner: abi::OutputSchema {
                id: id.to_string(),
                amount_sat,
                lock: lock.inner.clone(),
                asset: asset.inner.clone(),
                blinder: blinder.inner.clone(),
            },
        }
    }

    /// Return the output identifier.
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Return the output amount in satoshi.
    #[wasm_bindgen(js_name = amountSat)]
    pub fn amount_sat(&self) -> u64 {
        self.inner.amount_sat
    }

    /// Return the output lock variant.
    pub fn lock(&self) -> WalletAbiLockVariant {
        WalletAbiLockVariant {
            inner: self.inner.lock.clone(),
        }
    }

    /// Return the output asset variant.
    pub fn asset(&self) -> WalletAbiAssetVariant {
        WalletAbiAssetVariant {
            inner: self.inner.asset.clone(),
        }
    }

    /// Return the output blinder variant.
    pub fn blinder(&self) -> WalletAbiBlinderVariant {
        WalletAbiBlinderVariant {
            inner: self.inner.blinder.clone(),
        }
    }
}

/// Runtime parameters for a Wallet ABI transaction creation request.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiRuntimeParams {
    inner: abi::RuntimeParams,
}

#[wasm_bindgen]
impl WalletAbiRuntimeParams {
    fn from_json_str(json: &str) -> Result<WalletAbiRuntimeParams, crate::Error> {
        json_from_str(json).map(|inner| Self { inner })
    }
}

#[wasm_bindgen]
impl WalletAbiRuntimeParams {
    /// Build runtime parameters from inputs, outputs, and optional fee settings.
    pub fn new(
        inputs: Vec<WalletAbiInputSchema>,
        outputs: Vec<WalletAbiOutputSchema>,
        fee_rate_sat_kvb: Option<f32>,
        lock_time: Option<LockTime>,
    ) -> WalletAbiRuntimeParams {
        Self {
            inner: abi::RuntimeParams {
                inputs: inputs.into_iter().map(|input| input.inner).collect(),
                outputs: outputs.into_iter().map(|output| output.inner).collect(),
                fee_rate_sat_kvb,
                lock_time: lock_time.map(Into::into),
            },
        }
    }

    /// Parse canonical Wallet ABI runtime params JSON.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<WalletAbiRuntimeParams, crate::Error> {
        json_from_js_value(json).map(|inner| Self { inner })
    }

    /// Serialize this runtime params payload to canonical Wallet ABI JSON.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, crate::Error> {
        js_value_from_json(&self.inner)
    }

    /// Return the canonical JSON string for this runtime params payload.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        serde_json::to_string(&self.inner).expect("runtime params contain simple data")
    }

    /// Return the runtime input list.
    pub fn inputs(&self) -> Vec<WalletAbiInputSchema> {
        self.inner
            .inputs
            .iter()
            .cloned()
            .map(|inner| WalletAbiInputSchema { inner })
            .collect()
    }

    /// Return the runtime output list.
    pub fn outputs(&self) -> Vec<WalletAbiOutputSchema> {
        self.inner
            .outputs
            .iter()
            .cloned()
            .map(|inner| WalletAbiOutputSchema { inner })
            .collect()
    }

    /// Return the fee rate in satoshi per kvB when present.
    #[wasm_bindgen(js_name = feeRateSatKvb)]
    pub fn fee_rate_sat_kvb(&self) -> Option<f32> {
        self.inner.fee_rate_sat_kvb
    }

    /// Return the lock time when present.
    #[wasm_bindgen(js_name = lockTime)]
    pub fn lock_time(&self) -> Option<LockTime> {
        self.inner.lock_time.map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WalletAbiAssetVariant, WalletAbiBlinderVariant, WalletAbiLockVariant,
        WalletAbiOutputSchema, WalletAbiRuntimeParams,
    };

    use crate::{
        LockTime, Network, PublicKey, Script, SecretKey, TxSequence, WalletAbiFinalizerSpec,
        WalletAbiInputSchema, WalletAbiInputUnblinding, WalletAbiUtxoSource,
        WalletAbiWalletSourceFilter,
    };

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

    #[test]
    fn wallet_abi_blinder_variant_roundtrip() {
        let public_key = PublicKey::from_secret_key(
            &SecretKey::from_bytes(&[3_u8; 32]).expect("secret key"),
        );
        let provided = WalletAbiBlinderVariant::provided(&public_key).expect("provided blinder");

        assert_eq!(WalletAbiBlinderVariant::wallet().kind(), "wallet");
        assert_eq!(WalletAbiBlinderVariant::wallet().provided_pubkey(), None);
        assert_eq!(provided.kind(), "provided");
        assert_eq!(
            provided.provided_pubkey().expect("provided pubkey").to_string(),
            public_key.to_string()
        );
        assert_eq!(WalletAbiBlinderVariant::explicit().kind(), "explicit");
    }

    #[test]
    fn wallet_abi_output_schema_roundtrip() {
        let policy_asset = Network::testnet().policy_asset();
        let output = WalletAbiOutputSchema::new(
            "change",
            1_500,
            &WalletAbiLockVariant::script(&Script::new("6a").expect("script")),
            &WalletAbiAssetVariant::asset_id(&policy_asset),
            &WalletAbiBlinderVariant::explicit(),
        );

        assert_eq!(output.id(), "change".to_string());
        assert_eq!(output.amount_sat(), 1_500);
        assert_eq!(output.lock().kind(), "script");
        assert_eq!(output.asset().asset_id_value(), Some(policy_asset));
        assert_eq!(output.blinder().kind(), "explicit");
    }

    #[test]
    fn wallet_abi_runtime_params_roundtrip() {
        let policy_asset = Network::testnet().policy_asset();
        let params = WalletAbiRuntimeParams::new(
            vec![WalletAbiInputSchema::from_sequence(
                "i0",
                &WalletAbiUtxoSource::wallet(&WalletAbiWalletSourceFilter::any()),
                &WalletAbiInputUnblinding::wallet(),
                &TxSequence::zero(),
                &WalletAbiFinalizerSpec::wallet(),
            )],
            vec![WalletAbiOutputSchema::new(
                "o0",
                1_500,
                &WalletAbiLockVariant::wallet(),
                &WalletAbiAssetVariant::asset_id(&policy_asset),
                &WalletAbiBlinderVariant::explicit(),
            )],
            Some(123.0),
            Some(LockTime::from_consensus(42)),
        );

        let json = params.to_string_js();
        let decoded = WalletAbiRuntimeParams::from_json_str(&json).expect("deserialize params");

        assert_eq!(decoded.inputs()[0].id(), "i0".to_string());
        assert_eq!(decoded.outputs()[0].id(), "o0".to_string());
        assert_eq!(decoded.fee_rate_sat_kvb(), Some(123.0));
        assert_eq!(
            decoded.lock_time().expect("lock time").to_consensus_u32(),
            42
        );
        assert!(json.contains("\"fee_rate_sat_kvb\":123.0"));
    }
}
