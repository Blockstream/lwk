//! Liquid transaction output secrets

#[cfg(feature = "simplicity")]
use std::sync::Arc;

#[cfg(feature = "simplicity")]
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

#[cfg(feature = "simplicity")]
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

    /// Return the asset blinding factor.
    pub fn asset_blinding_factor(&self) -> Arc<types::AssetBlindingFactor> {
        Arc::new(self.inner.asset_bf.into())
    }

    /// Return the value blinding factor.
    pub fn value_blinding_factor(&self) -> Arc<types::ValueBlindingFactor> {
        Arc::new(self.inner.value_bf.into())
    }
}

#[uniffi::export]
impl TxOutSecrets {
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
    #[cfg(feature = "simplicity")]
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
    #[cfg(feature = "simplicity")]
    use crate::{types, TxOutSecrets};

    use std::str::FromStr;

    use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use elements::hex::FromHex;
    use elements::AssetId;

    #[cfg(feature = "simplicity")]
    use elements::hex::ToHex;

    #[cfg(feature = "simplicity")]
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
        let asset_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let abf_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471";
        let vbf_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a472";

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
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_explicit.value_blinding_factor().to_string(),
            zero_hex,
        );
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_explicit.asset_blinding_factor().to_string(),
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
        let vc_hex = "099be562150e1db49df61c175dad7927c41152f19578ce09660b5346ea9e31e576";
        let ac_hex = "0baffb010ebadaefcf81343637e2abf439fc2615ce30a8c4157ec36a11f926acb1";

        assert!(!txoutsecrets_blinded.is_explicit());
        assert_eq!(txoutsecrets_blinded.value(), 1000);
        assert_eq!(txoutsecrets_blinded.asset().to_string(), asset_hex,);
        assert_eq!(txoutsecrets_blinded.asset_bf().to_string(), abf_hex,);
        assert_eq!(txoutsecrets_blinded.value_bf().to_string(), vbf_hex,);
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_blinded.asset_blinding_factor().to_string(),
            abf_hex,
        );
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_blinded.value_blinding_factor().to_string(),
            vbf_hex,
        );
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_blinded
                .asset_blinding_factor()
                .to_bytes()
                .to_hex(),
            reverse_hex(abf_hex),
        );
        #[cfg(feature = "simplicity")]
        assert_eq!(
            txoutsecrets_blinded
                .value_blinding_factor()
                .to_bytes()
                .to_hex(),
            reverse_hex(vbf_hex),
        );
        assert_eq!(txoutsecrets_blinded.asset_commitment().to_string(), ac_hex);
        assert_eq!(txoutsecrets_blinded.value_commitment().to_string(), vc_hex);
    }

    #[test]
    #[cfg(feature = "simplicity")]
    fn test_tx_out_secrets_new_with_blinding() {
        let asset_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let abf_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471";
        let vbf_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a472";

        let asset_id: types::AssetId = elements::AssetId::from_str(asset_hex).unwrap().into();
        let asset_bf = types::AssetBlindingFactor::from_string(abf_hex).unwrap();
        let value_bf = types::ValueBlindingFactor::from_string(vbf_hex).unwrap();

        let secrets = TxOutSecrets::new(asset_id, &asset_bf, 1000, &value_bf);

        assert!(!secrets.is_explicit());
        assert_eq!(secrets.asset().to_string(), asset_hex);
        assert_eq!(secrets.asset_bf().to_string(), abf_hex);
        assert_eq!(secrets.asset_blinding_factor().to_string(), abf_hex);
        assert_eq!(secrets.value(), 1000);
        assert_eq!(secrets.value_bf().to_string(), vbf_hex);
        assert_eq!(secrets.value_blinding_factor().to_string(), vbf_hex);
    }
}
