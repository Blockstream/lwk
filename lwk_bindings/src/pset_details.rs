use std::sync::Arc;

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

impl PsetBalance {
    pub fn fee(&self) -> u64 {
        self.inner.fee
    }
}

#[cfg(test)]
mod tests {
    use crate::{Network, Pset, Wollet, WolletDescriptor};

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
    }
}
