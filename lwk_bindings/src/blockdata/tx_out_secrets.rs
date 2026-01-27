//! Liquid transaction output secrets

use std::sync::Arc;

use crate::types;
use crate::types::{AssetId, Hex};
use elements::secp256k1_zkp::{Generator, PedersenCommitment, Tag};
use lwk_wollet::EC;

/// Contains unblinded information such as the asset and the value of a transaction output
#[derive(uniffi::Object, PartialEq, Eq, Debug)]
pub struct TxOutSecrets {
    inner: elements::TxOutSecrets,
}

impl From<elements::TxOutSecrets> for TxOutSecrets {
    fn from(inner: elements::TxOutSecrets) -> Self {
        Self { inner }
    }
}

impl From<&TxOutSecrets> for elements::TxOutSecrets {
    fn from(value: &TxOutSecrets) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl TxOutSecrets {
    /// Create TxOutSecrets with explicit blinding factors.
    #[uniffi::constructor]
    pub fn new(
        asset_id: AssetId,
        asset_bf: &types::AssetBlindingFactor,
        value: u64,
        value_bf: &types::ValueBlindingFactor,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: elements::TxOutSecrets::new(
                asset_id.into(),
                asset_bf.into(),
                value,
                value_bf.into(),
            ),
        })
    }

    /// Create TxOutSecrets from explicit (unblinded) values.
    #[uniffi::constructor]
    pub fn from_explicit(asset_id: AssetId, value: u64) -> Arc<Self> {
        Arc::new(Self {
            inner: elements::TxOutSecrets::new(
                asset_id.into(),
                elements::confidential::AssetBlindingFactor::zero(),
                value,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
        })
    }

    /// Return the asset identifier of the output.
    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }

    /// Return the asset blinding factor as a hex string.
    ///
    /// Deprecated: use `asset_blinding_factor()` instead.
    pub fn asset_bf(&self) -> Hex {
        self.inner
            .asset_bf
            .to_string()
            .parse()
            .expect("asset_bf to_string creates valid hex")
    }

    /// Return the asset blinding factor.
    pub fn asset_blinding_factor(&self) -> Arc<types::AssetBlindingFactor> {
        Arc::new(self.inner.asset_bf.into())
    }

    /// Return the value of the output.
    pub fn value(&self) -> u64 {
        self.inner.value
    }

    /// Return the value blinding factor as a hex string.
    ///
    /// Deprecated: use `value_blinding_factor()` instead.
    pub fn value_bf(&self) -> Hex {
        self.inner
            .value_bf
            .to_string()
            .parse()
            .expect("value_bf to_string creates valid hex")
    }

    /// Return the value blinding factor.
    pub fn value_blinding_factor(&self) -> Arc<types::ValueBlindingFactor> {
        Arc::new(self.inner.value_bf.into())
    }

    /// Return true if the output is explicit (no blinding factors).
    pub fn is_explicit(&self) -> bool {
        self.inner.asset_bf == elements::confidential::AssetBlindingFactor::zero()
            && self.inner.value_bf == elements::confidential::ValueBlindingFactor::zero()
    }

    /// Get the asset commitment
    ///
    /// If the output is explicit, returns the empty string
    pub fn asset_commitment(&self) -> Hex {
        if self.is_explicit() {
            "".parse().expect("empty string")
        } else {
            self.asset_generator()
                .to_string()
                .parse()
                .expect("from pedersen commitment")
        }
    }

    /// Get the value commitment
    ///
    /// If the output is explicit, returns the empty string
    pub fn value_commitment(&self) -> Hex {
        if self.is_explicit() {
            "".parse().expect("empty string")
        } else {
            let value = self.inner.value;
            let vbf = self.inner.value_bf.into_inner();

            PedersenCommitment::new(&EC, value, vbf, self.asset_generator())
                .to_string()
                .parse()
                .expect("from pedersen commitment")
        }
    }
}

impl TxOutSecrets {
    pub(crate) fn inner(&self) -> &elements::TxOutSecrets {
        &self.inner
    }

    fn asset_generator(&self) -> Generator {
        let asset = self.inner.asset.into_inner().to_byte_array();
        let abf = self.inner.asset_bf.into_inner();
        let asset_tag = Tag::from(asset);
        Generator::new_blinded(&EC, asset_tag, abf)
    }
}

#[cfg(test)]
mod tests {
    use crate::UniffiCustomTypeConverter;
    use crate::{types, TxOutSecrets};

    use std::str::FromStr;

    use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use elements::hex::FromHex;
    use elements::AssetId;

    fn reverse_hex(hex: &str) -> String {
        hex.as_bytes()
            .chunks(2)
            .rev()
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect()
    }

    #[test]
    fn tx_out_secrets() {
        let zero_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let asset_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let abf_hex = "0102030405060708091011121314151617181920212223242526272829303132";
        let vbf_hex = "aabbccdd00112233445566778899aabb01020304050607080910111213141516";

        let txoutsecrets_explicit: crate::TxOutSecrets = elements::TxOutSecrets::new(
            AssetId::from_str(asset_hex).unwrap(),
            AssetBlindingFactor::zero(),
            1000,
            ValueBlindingFactor::zero(),
        )
        .into();

        assert!(txoutsecrets_explicit.is_explicit());
        assert_eq!(txoutsecrets_explicit.value(), 1000);
        assert_eq!(txoutsecrets_explicit.asset().to_string(), asset_hex,);
        assert_eq!(txoutsecrets_explicit.value_bf().to_string(), zero_hex,);
        assert_eq!(txoutsecrets_explicit.asset_bf().to_string(), zero_hex,);
        assert_eq!(
            txoutsecrets_explicit.value_blinding_factor().to_hex(),
            zero_hex,
        );
        assert_eq!(
            txoutsecrets_explicit.asset_blinding_factor().to_hex(),
            zero_hex,
        );
        assert_eq!(txoutsecrets_explicit.asset_commitment().to_string(), "");
        assert_eq!(txoutsecrets_explicit.value_commitment().to_string(), "");

        let txoutsecrets_blinded: crate::TxOutSecrets = elements::TxOutSecrets::new(
            AssetId::from_str(asset_hex).unwrap(),
            AssetBlindingFactor::from_hex(abf_hex).unwrap(),
            1000,
            ValueBlindingFactor::from_hex(vbf_hex).unwrap(),
        )
        .into();
        let vc_hex = "08e092ca785f8d07681db07467e05f585e562bcf47171ddbe74d0c825f49c535fe";
        let ac_hex = "0b73d08a80d4df97c7917eb231d2d9949422e49d5243e3b7342cfb7f409d05fae6";

        assert!(!txoutsecrets_blinded.is_explicit());
        assert_eq!(txoutsecrets_blinded.value(), 1000);
        assert_eq!(txoutsecrets_blinded.asset().to_string(), asset_hex,);
        assert_eq!(txoutsecrets_blinded.asset_bf().to_string(), abf_hex,);
        assert_eq!(txoutsecrets_blinded.value_bf().to_string(), vbf_hex,);
        assert_eq!(
            txoutsecrets_blinded.asset_blinding_factor().to_string(),
            abf_hex,
        );
        assert_eq!(
            txoutsecrets_blinded.value_blinding_factor().to_string(),
            vbf_hex,
        );
        assert_eq!(
            txoutsecrets_blinded.asset_blinding_factor().to_hex(),
            reverse_hex(abf_hex),
        );
        assert_eq!(
            txoutsecrets_blinded.value_blinding_factor().to_hex(),
            reverse_hex(vbf_hex),
        );
        assert_eq!(txoutsecrets_blinded.asset_commitment().to_string(), ac_hex);
        assert_eq!(txoutsecrets_blinded.value_commitment().to_string(), vc_hex);
    }

    #[test]
    fn test_tx_out_secrets_new_with_blinding() {
        let asset_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let abf_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let vbf_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        let asset_id = types::AssetId::into_custom(asset_hex.to_string()).unwrap();
        let asset_bf = types::AssetBlindingFactor::from_hex(abf_hex).unwrap();
        let value_bf = types::ValueBlindingFactor::from_hex(vbf_hex).unwrap();

        let secrets = TxOutSecrets::new(asset_id, &asset_bf, 1000, &value_bf);

        assert!(!secrets.is_explicit());
        assert_eq!(secrets.asset().to_string(), asset_hex);
        assert_eq!(secrets.asset_bf().to_string(), abf_hex);
        assert_eq!(secrets.asset_blinding_factor().to_hex(), abf_hex);
        assert_eq!(secrets.value(), 1000);
        assert_eq!(secrets.value_bf().to_string(), vbf_hex);
        assert_eq!(secrets.value_blinding_factor().to_hex(), vbf_hex);
    }
}
