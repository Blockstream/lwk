use std::sync::Arc;

use crate::{Address, PublicKey, Script};

/// Wallet-owned output template returned to the runtime for receive/change outputs.
#[derive(uniffi::Record, Clone)]
pub struct WalletAbiWalletOutputTemplate {
    /// Script pubkey chosen by the wallet descriptor/policy.
    pub script_pubkey: Arc<Script>,
    /// Optional confidential blinding public key paired with `script_pubkey`.
    pub blinding_pubkey: Option<Arc<PublicKey>>,
}

impl From<&lwk_simplicity::wallet_abi::WalletOutputTemplate> for WalletAbiWalletOutputTemplate {
    fn from(value: &lwk_simplicity::wallet_abi::WalletOutputTemplate) -> Self {
        Self {
            script_pubkey: Arc::new(value.script_pubkey.clone().into()),
            blinding_pubkey: value
                .blinding_pubkey
                .map(|key| Arc::new(elements::bitcoin::PublicKey::new(key).into())),
        }
    }
}

/// Build a wallet output template from a wallet-owned address.
#[uniffi::export]
pub fn wallet_abi_output_template_from_address(
    address: &Address,
) -> WalletAbiWalletOutputTemplate {
    WalletAbiWalletOutputTemplate {
        script_pubkey: address.script_pubkey(),
        blinding_pubkey: address
            .as_ref()
            .blinding_pubkey
            .map(|key| Arc::new(elements::bitcoin::PublicKey::new(key).into())),
    }
}

#[cfg(test)]
mod tests {
    use super::{wallet_abi_output_template_from_address, WalletAbiWalletOutputTemplate};
    use crate::Address;

    #[test]
    fn wallet_abi_output_template_from_address_keeps_script_and_blinder() {
        let address = Address::new(
            "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
        )
        .expect("address");

        let template = wallet_abi_output_template_from_address(&address);
        let inner_address: &elements::Address = address.as_ref().as_ref();

        assert_eq!(
            template.script_pubkey.to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );
        assert_eq!(
            template.blinding_pubkey.expect("blinding pubkey").to_string(),
            elements::bitcoin::PublicKey::new(
                inner_address.blinding_pubkey.expect("address blinder"),
            )
            .to_string()
        );
    }

    #[test]
    fn wallet_abi_wallet_output_template_maps_runtime_template() {
        let runtime_template = lwk_simplicity::wallet_abi::WalletOutputTemplate {
            script_pubkey: elements::Script::new(),
            blinding_pubkey: None,
        };

        let template = WalletAbiWalletOutputTemplate::from(&runtime_template);

        assert_eq!(template.script_pubkey.to_string(), "");
        assert_eq!(template.blinding_pubkey, None);
    }
}
