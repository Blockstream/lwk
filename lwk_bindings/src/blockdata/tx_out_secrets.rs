use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::secp256k1_zkp::{Generator, PedersenCommitment, Tag};
use lwk_wollet::EC;

use crate::types::{AssetId, Hex};

#[derive(uniffi::Object, PartialEq, Eq, Debug)]
pub struct TxOutSecrets {
    inner: elements::TxOutSecrets,
}

impl From<elements::TxOutSecrets> for TxOutSecrets {
    fn from(inner: elements::TxOutSecrets) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl TxOutSecrets {
    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }

    pub fn asset_bf(&self) -> Hex {
        self.inner
            .asset_bf
            .to_string()
            .parse()
            .expect("asset_bf to_string creates valid hex")
    }

    pub fn value(&self) -> u64 {
        self.inner.value
    }

    pub fn value_bf(&self) -> Hex {
        self.inner
            .value_bf
            .to_string()
            .parse()
            .expect("value_bf to_string creates valid hex")
    }

    pub fn is_explicit(&self) -> bool {
        self.inner.asset_bf == AssetBlindingFactor::zero()
            && self.inner.value_bf == ValueBlindingFactor::zero()
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

    #[test]
    fn tx_out_secrets() {
        let elements_tx_out_secrets = elements::TxOutSecrets::new(
            elements::AssetId::default(),
            elements::confidential::AssetBlindingFactor::zero(),
            1000,
            elements::confidential::ValueBlindingFactor::zero(),
        );
        let tx_out_secrets: crate::TxOutSecrets = elements_tx_out_secrets.into();

        assert_eq!(tx_out_secrets.value(), 1000);
        assert_eq!(
            tx_out_secrets.value_bf().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );

        assert_eq!(
            tx_out_secrets.asset().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            tx_out_secrets.asset_bf().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
