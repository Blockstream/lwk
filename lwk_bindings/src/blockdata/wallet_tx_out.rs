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
        Arc::new(self.inner.address().into())
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
    use lwk_wollet::ElementsNetwork;

    use super::WalletTxOut;

    #[test]
    fn wallet_tx_out() {
        let address_str = "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";
        let definite_descriptor = "ct(slip77(e574b56c3f770be325b48770537cab2278c740352dfb010f4756b5562be12e6e),elwpkh([7a414e60/84'/1'/0']tpubDDRxgt3k7isfqd26r8m3qiWa2DWghshZdCCpxPBWhtxP5oBw29cczWLTt9rv5TnwA9yTnfGGB32mdumHSgN9sgbttZV7gbCX5M6eAzxXJBB/0/0))#jlg2w5v2".to_string();

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
            definite_descriptor,
            network: ElementsNetwork::LiquidTestnet,
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
