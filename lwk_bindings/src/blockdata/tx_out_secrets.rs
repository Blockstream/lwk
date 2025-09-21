//! Liquid transaction output secrets

use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::secp256k1_zkp::{Generator, PedersenCommitment, Tag};
use lwk_wollet::EC;

use crate::types::{AssetId, Hex};

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
    /// Return the asset identifier of the output.
    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }

    /// Return the asset blinding factor as a hex string.
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
    pub fn value_bf(&self) -> Hex {
        self.inner
            .value_bf
            .to_string()
            .parse()
            .expect("value_bf to_string creates valid hex")
    }

    /// Return true if the output is explicit (no blinding factors).
    pub fn is_explicit(&self) -> bool {
        self.inner.asset_bf == AssetBlindingFactor::zero()
            && self.inner.value_bf == ValueBlindingFactor::zero()
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
    fn asset_generator(&self) -> Generator {
        let asset = self.inner.asset.into_inner().to_byte_array();
        let abf = self.inner.asset_bf.into_inner();
        let asset_tag = Tag::from(asset);
        Generator::new_blinded(&EC, asset_tag, abf)
    }
}

#[cfg(test)]
mod tests {
    use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use elements::hex::FromHex;
    use elements::AssetId;
    use std::str::FromStr;

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
        assert_eq!(txoutsecrets_explicit.value_bf().to_string(), zero_hex,);
        assert_eq!(txoutsecrets_explicit.asset_bf().to_string(), zero_hex,);
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
        assert_eq!(txoutsecrets_blinded.asset_bf().to_string(), abf_hex,);
        assert_eq!(txoutsecrets_blinded.value_bf().to_string(), vbf_hex,);
        assert_eq!(txoutsecrets_blinded.asset_commitment().to_string(), ac_hex);
        assert_eq!(txoutsecrets_blinded.value_commitment().to_string(), vc_hex);
    }
}
