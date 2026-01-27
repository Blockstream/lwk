//! Liquid transaction output secrets

use std::sync::Arc;

use elements::secp256k1_zkp::{Generator, PedersenCommitment, Tag};
use lwk_wollet::EC;

use crate::types::{
    AssetBlindingFactor as BindingsAssetBlindingFactor, AssetId, Hex,
    ValueBlindingFactor as BindingsValueBlindingFactor,
};

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
        asset_bf: &BindingsAssetBlindingFactor,
        value: u64,
        value_bf: &BindingsValueBlindingFactor,
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

    /// Return the asset blinding factor.
    pub fn asset_bf(&self) -> Arc<BindingsAssetBlindingFactor> {
        Arc::new(self.inner.asset_bf.into())
    }

    /// Return the value of the output.
    pub fn value(&self) -> u64 {
        self.inner.value
    }

    /// Return the value blinding factor.
    pub fn value_bf(&self) -> Arc<BindingsValueBlindingFactor> {
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

    #[test]
    fn tx_out_secrets() {
        let zero_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let asset_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let abf_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let vbf_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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
        assert_eq!(txoutsecrets_explicit.value_bf().to_hex(), zero_hex,);
        assert_eq!(txoutsecrets_explicit.asset_bf().to_hex(), zero_hex,);
        assert_eq!(txoutsecrets_explicit.asset_commitment().to_string(), "");
        assert_eq!(txoutsecrets_explicit.value_commitment().to_string(), "");

        let txoutsecrets_blinded: crate::TxOutSecrets = elements::TxOutSecrets::new(
            AssetId::from_str(asset_hex).unwrap(),
            AssetBlindingFactor::from_hex(abf_hex).unwrap(),
            1000,
            ValueBlindingFactor::from_hex(vbf_hex).unwrap(),
        )
        .into();
        let vc_hex = "08b3bfb93e411bf83c5095c44c5f1a8fa9da4bf5978b20dacff7fe594b896d352a";
        let ac_hex = "0b9bccef298a184a714e09656fe1596ab1a5b7e70b5d7b71ef0cb7d069a755cd3e";

        assert!(!txoutsecrets_blinded.is_explicit());
        assert_eq!(txoutsecrets_blinded.value(), 1000);
        assert_eq!(txoutsecrets_blinded.asset().to_string(), asset_hex,);
        assert_eq!(txoutsecrets_blinded.asset_bf().to_hex(), abf_hex,);
        assert_eq!(txoutsecrets_blinded.value_bf().to_hex(), vbf_hex,);
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
        assert_eq!(secrets.asset_bf().to_hex(), abf_hex);
        assert_eq!(secrets.value(), 1000);
        assert_eq!(secrets.value_bf().to_hex(), vbf_hex);
    }
}
