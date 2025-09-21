//! Liquid wallet transaction output

use std::sync::Arc;

use crate::{Address, Chain, OutPoint, Script, TxOutSecrets};

/// Details of a wallet transaction output used in `WalletTx`
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
    /// Return the outpoint (txid and vout) of this `WalletTxOut`.
    pub fn outpoint(&self) -> Arc<OutPoint> {
        Arc::new(self.inner.outpoint.into())
    }

    /// Return the script pubkey of the address of this `WalletTxOut`.
    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey.clone().into())
    }

    /// Return the height of the block containing this output if it's confirmed.
    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    /// Return the address of this `WalletTxOut`.
    pub fn address(&self) -> Arc<Address> {
        Arc::new(self.inner.address.clone().into())
    }

    /// Return the unblinded values of this `WalletTxOut`.
    pub fn unblinded(&self) -> Arc<TxOutSecrets> {
        Arc::new(self.inner.unblinded.into())
    }

    /// Return the wildcard index used to derive the address of this `WalletTxOut`.
    pub fn wildcard_index(&self) -> u32 {
        self.inner.wildcard_index
    }

    /// Return the chain of this `WalletTxOut`. Can be "Chain::External" or "Chain::Internal" (change).
    pub fn ext_int(&self) -> Chain {
        self.inner.ext_int.into()
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use elements::{hex::ToHex, Address};

    use super::WalletTxOut;

    #[test]
    fn wallet_tx_out() {
        let address_str = "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";
        let address = Address::from_str(address_str).unwrap();

        let el = lwk_wollet::WalletTxOut {
            is_spent: false,
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
            address,
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
