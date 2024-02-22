use crate::{OutPoint, Script, TxOutSecrets};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WalletTxOut {
    inner: lwk_wollet::WalletTxOut,
}

impl From<lwk_wollet::WalletTxOut> for WalletTxOut {
    fn from(inner: lwk_wollet::WalletTxOut) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl WalletTxOut {
    pub fn outpoint(&self) -> OutPoint {
        self.inner.outpoint.into()
    }

    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey.clone().into()
    }

    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    pub fn unblinded(&self) -> TxOutSecrets {
        self.inner.unblinded.into()
    }

    pub fn wildcard_index(&self) -> u32 {
        self.inner.wildcard_index
    }

    // TODO Chain type
    // pub fn ext_int(&self) -> Chain {
    //     self.inner.ext_int.into()
    // }
}

/// An optional wallet transaction output. Could be None when it's not possible to unblind.
/// It seems required by wasm_bindgen because we can't return `Vec<Option<WalletTxOut>>`
#[wasm_bindgen]
pub struct OptionWalletTxOut {
    inner: Option<WalletTxOut>,
}

impl From<Option<lwk_wollet::WalletTxOut>> for OptionWalletTxOut {
    fn from(inner: Option<lwk_wollet::WalletTxOut>) -> Self {
        Self {
            inner: inner.map(Into::into),
        }
    }
}

#[wasm_bindgen]
impl OptionWalletTxOut {
    pub fn get(&self) -> Option<WalletTxOut> {
        self.inner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::WalletTxOut;
    use lwk_wollet::elements::{self, hex::ToHex};
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn wallet_tx_out() {
        let el = lwk_wollet::WalletTxOut {
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

        assert_eq!(wallet_tx_out.unblinded(), el.unblinded.into());

        assert_eq!(wallet_tx_out.wildcard_index(), el.wildcard_index);

        // assert_eq!(wallet_tx_out.ext_int(), el.ext_int.into());
    }
}
