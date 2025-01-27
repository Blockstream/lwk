use std::{collections::HashMap, sync::Arc};

use crate::types::AssetId;

#[derive(uniffi::Object, Debug)]
pub struct PsetDetails {
    inner: lwk_common::PsetDetails,
}

impl From<lwk_common::PsetDetails> for PsetDetails {
    fn from(inner: lwk_common::PsetDetails) -> Self {
        Self { inner }
    }
}

impl PsetDetails {
    pub fn balance(&self) -> Arc<PsetBalance> {
        Arc::new(self.inner.balance.clone().into())
    }
}

#[derive(uniffi::Object, Debug)]
pub struct PsetBalance {
    inner: lwk_common::PsetBalance,
}

impl From<lwk_common::PsetBalance> for PsetBalance {
    fn from(inner: lwk_common::PsetBalance) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl PsetBalance {
    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    pub fn balances(&self) -> HashMap<AssetId, i64> {
        self.inner
            .balances
            .iter()
            .map(|(k, v)| ((*k).into(), *v))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{types::AssetId, Network, Pset, Wollet, WolletDescriptor};

    #[test]
    fn pset_details() {
        let pset = include_str!("../test_data/pset_details/pset.base64");
        let pset = Pset::new(pset).unwrap();

        let descriptor = include_str!("../test_data/pset_details/desc");
        let descriptor = WolletDescriptor::new(descriptor).unwrap();
        let network = Network::regtest_default();
        let wollet = Wollet::new(&network, &descriptor, None).unwrap();

        let details = wollet.pset_details(&pset).unwrap();
        assert_eq!(details.balance().fee(), 254);

        let balances = details.balance().balances();
        assert_eq!(balances.len(), 1);
        let expected_asset_id = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset_id = elements::AssetId::from_str(&expected_asset_id).unwrap();
        let asset_id: AssetId = asset_id.into();
        let val = balances.get(&asset_id).unwrap();
        assert_eq!(*val, -1254);
    }
}
