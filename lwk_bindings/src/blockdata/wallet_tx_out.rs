use std::sync::Arc;

use crate::{Address, Chain, OutPoint, Script, TxOutSecrets};

#[derive(uniffi::Object)]
pub struct WalletTxOut {
    inner: lwk_wollet::WalletTxOut,
}

impl From<lwk_wollet::WalletTxOut> for WalletTxOut {
    fn from(inner: lwk_wollet::WalletTxOut) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl WalletTxOut {
    pub fn outpoint(&self) -> Arc<OutPoint> {
        Arc::new(self.inner.outpoint.into())
    }

    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey.clone().into())
    }

    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    pub fn address(&self) -> Arc<Address> {
        Arc::new(self.inner.address.clone().into())
    }

    pub fn unblinded(&self) -> Arc<TxOutSecrets> {
        Arc::new(self.inner.unblinded.into())
    }

    pub fn wildcard_index(&self) -> u32 {
        self.inner.wildcard_index
    }

    pub fn ext_int(&self) -> Chain {
        self.inner.ext_int.into()
    }
}

#[cfg(test)]
mod tests {

    use elements::hex::ToHex;

    use super::WalletTxOut;

    #[test]
    fn wallet_tx_out() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        let address: elements::Address = address_str.parse().unwrap();

        let el = lwk_wollet::WalletTxOut {
            is_spent: false,
            address,
            outpoint: elements::OutPoint::null(),
            script_pubkey: elements::Script::new(),
            height: Some(1),
            unblinded: elements::TxOutSecrets::new(
                elements::AssetId::default(),
                elements::confidential::AssetBlindingFactor::zero(),
                1000,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
            wildcard_index: 10,
            ext_int: lwk_wollet::Chain::External,
        };

        let wallet_tx_out: WalletTxOut = el.clone().into();

        assert_eq!(
            wallet_tx_out.outpoint().to_string(),
            el.outpoint.to_string()
        );

        assert_eq!(
            wallet_tx_out.script_pubkey().to_string(),
            el.script_pubkey.to_hex()
        );

        assert_eq!(wallet_tx_out.height(), el.height);

        assert_eq!(*wallet_tx_out.unblinded(), el.unblinded.into());

        assert_eq!(wallet_tx_out.wildcard_index(), el.wildcard_index);

        assert_eq!(wallet_tx_out.ext_int(), el.ext_int.into());

        assert_eq!(wallet_tx_out.address().to_string(), address_str);
    }
}
