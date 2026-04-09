use super::filters::WalletAbiFinalizerSpec;

use crate::Script;

use lwk_simplicity::wallet_abi::schema as abi;

use wasm_bindgen::prelude::*;

/// A Wallet ABI output lock variant.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiLockVariant {
    inner: abi::LockVariant,
}

#[wasm_bindgen]
impl WalletAbiLockVariant {
    /// Build the Wallet ABI `wallet` lock variant.
    pub fn wallet() -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Wallet,
        }
    }

    /// Build the Wallet ABI `script` lock variant.
    pub fn script(script: &Script) -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Script {
                script: script.as_ref().clone(),
            },
        }
    }

    /// Build the Wallet ABI `finalizer` lock variant.
    pub fn finalizer(finalizer: &WalletAbiFinalizerSpec) -> WalletAbiLockVariant {
        Self {
            inner: abi::LockVariant::Finalizer {
                finalizer: Box::new(finalizer.clone().inner),
            },
        }
    }

    /// Return the canonical Wallet ABI variant tag string.
    pub fn kind(&self) -> String {
        match self.inner {
            abi::LockVariant::Wallet => "wallet",
            abi::LockVariant::Script { .. } => "script",
            abi::LockVariant::Finalizer { .. } => "finalizer",
        }
        .to_string()
    }

    /// Return the script when this lock is the `script` variant.
    #[wasm_bindgen(js_name = scriptValue)]
    pub fn script_value(&self) -> Option<Script> {
        match &self.inner {
            abi::LockVariant::Script { script } => Some(script.clone().into()),
            abi::LockVariant::Wallet | abi::LockVariant::Finalizer { .. } => None,
        }
    }

    /// Return the finalizer when this lock is the `finalizer` variant.
    #[wasm_bindgen(js_name = finalizerValue)]
    pub fn finalizer_value(&self) -> Option<WalletAbiFinalizerSpec> {
        match &self.inner {
            abi::LockVariant::Finalizer { finalizer } => Some(WalletAbiFinalizerSpec {
                inner: (**finalizer).clone(),
            }),
            abi::LockVariant::Wallet | abi::LockVariant::Script { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiLockVariant;

    use crate::{Script, WalletAbiFinalizerSpec};

    #[test]
    fn wallet_abi_lock_variant_roundtrip() {
        let script = Script::new("6a").expect("script");
        let script_variant = WalletAbiLockVariant::script(&script);
        let finalizer_variant = WalletAbiLockVariant::finalizer(&WalletAbiFinalizerSpec::wallet());

        assert_eq!(WalletAbiLockVariant::wallet().kind(), "wallet");
        assert_eq!(script_variant.kind(), "script");
        assert_eq!(
            script_variant.script_value().expect("script value").to_string(),
            script.to_string()
        );
        assert!(script_variant.finalizer_value().is_none());
        assert_eq!(finalizer_variant.kind(), "finalizer");
        assert_eq!(
            finalizer_variant
                .finalizer_value()
                .expect("finalizer value")
                .kind(),
            "wallet"
        );
    }
}
