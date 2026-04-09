use crate::Error;

use std::str::FromStr;

use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;

use wasm_bindgen::prelude::*;

/// A canonical external taproot handle used by Wallet ABI callers.
#[wasm_bindgen]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletAbiTaprootHandle {
    inner: TaprootPubkeyGen,
}

impl std::fmt::Display for WalletAbiTaprootHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<TaprootPubkeyGen> for WalletAbiTaprootHandle {
    fn from(inner: TaprootPubkeyGen) -> Self {
        Self { inner }
    }
}

impl From<WalletAbiTaprootHandle> for TaprootPubkeyGen {
    fn from(value: WalletAbiTaprootHandle) -> Self {
        value.inner
    }
}

impl From<&WalletAbiTaprootHandle> for TaprootPubkeyGen {
    fn from(value: &WalletAbiTaprootHandle) -> Self {
        value.inner.clone()
    }
}

#[wasm_bindgen]
impl WalletAbiTaprootHandle {
    /// Parse the canonical `<seed_or_ext-xonly_hex>:<pubkey>:<address>` taproot-handle string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<WalletAbiTaprootHandle, Error> {
        TaprootPubkeyGen::from_str(s)
            .map(Into::into)
            .map_err(|error| Error::Generic(error.to_string()))
    }

    /// Return the canonical taproot-handle string.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiTaprootHandle;

    use std::str::FromStr;

    use lwk_common::Network;
    use lwk_simplicity::simplicityhl::elements::Address;
    use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;

    #[test]
    fn wallet_abi_taproot_handle_roundtrip() {
        let handle = TaprootPubkeyGen::from(
            &(),
            Network::Liquid,
            &|_, _, _| {
                Ok(Address::from_str("lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv")
                    .expect("valid fixed address"))
            },
        )
        .expect("build taproot handle")
        .to_string();
        let parsed = WalletAbiTaprootHandle::from_string(&handle).expect("parse handle");

        assert_eq!(parsed.to_string(), handle);
    }
}
