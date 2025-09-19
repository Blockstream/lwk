use crate::{Address, OutPoint, Script, TxOutSecrets};
use wasm_bindgen::prelude::*;

/// Details of a wallet transaction output used in `WalletTx`
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

/// Wallet chain
#[derive(Debug, PartialEq, Eq)]
#[wasm_bindgen]
pub enum Chain {
    /// External address, shown when asked for a payment.
    /// Wallet having a single descriptor are considered External
    External,

    /// Internal address, used for the change
    Internal,
}

impl From<lwk_wollet::Chain> for Chain {
    fn from(value: lwk_wollet::Chain) -> Self {
        match value {
            lwk_wollet::Chain::External => Chain::External,
            lwk_wollet::Chain::Internal => Chain::Internal,
        }
    }
}

#[wasm_bindgen]
impl WalletTxOut {
    /// Return the outpoint (txid and vout) of this `WalletTxOut`.
    pub fn outpoint(&self) -> OutPoint {
        self.inner.outpoint.into()
    }

    /// Return the script pubkey of the address of this `WalletTxOut`.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey.clone().into()
    }

    /// Return the height of the block containing this output if it's confirmed.
    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    /// Return the unblinded values of this `WalletTxOut`.
    pub fn unblinded(&self) -> TxOutSecrets {
        self.inner.unblinded.into()
    }

    /// Return the wildcard index used to derive the address of this `WalletTxOut`.
    #[wasm_bindgen(js_name = wildcardIndex)]
    pub fn wildcard_index(&self) -> u32 {
        self.inner.wildcard_index
    }

    /// Return the chain of this `WalletTxOut`. Can be "Chain::External" or "Chain::Internal" (change).
    #[wasm_bindgen(js_name = extInt)]
    pub fn ext_int(&self) -> Chain {
        self.inner.ext_int.into()
    }

    /// Return the address of this `WalletTxOut`.
    pub fn address(&self) -> Address {
        self.inner.address.clone().into()
    }
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
    /// Return a copy of the WalletTxOut if it exists, otherwise None
    pub fn get(&self) -> Option<WalletTxOut> {
        self.inner.clone()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::WalletTxOut;
    use lwk_wollet::elements::{self, hex::ToHex};
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn wallet_tx_out() {
        let address_str = "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";
        let address = crate::Address::new(address_str).unwrap();

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
            is_spent: false,
            address: address.into(),
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

        assert_eq!(wallet_tx_out.address().to_string(), address_str);

        // assert_eq!(wallet_tx_out.ext_int(), el.ext_int.into());
    }
}
