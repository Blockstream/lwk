use std::sync::Arc;

use crate::{
    wallet_abi::request_session::session_from_runtime, LwkError, WalletAbiRequestSession,
    WalletAbiWalletOutputRequest, WalletAbiWalletOutputTemplate,
};

use lwk_simplicity::wallet_abi::{
    WalletOutputAllocator, WalletOutputRequest, WalletOutputTemplate,
};

/// Foreign callback surface for deterministic wallet output allocation.
#[uniffi::export(with_foreign)]
pub trait WalletAbiOutputAllocatorCallbacks: Send + Sync {
    /// Return the wallet-owned output template for a deterministic request selector.
    fn get_wallet_output_template(
        &self,
        session: WalletAbiRequestSession,
        request: WalletAbiWalletOutputRequest,
    ) -> Result<WalletAbiWalletOutputTemplate, LwkError>;
}

/// Error type for the wallet output-allocator bridge.
#[derive(thiserror::Error, Debug)]
pub enum WalletOutputAllocatorLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
}

/// Bridge adapting foreign output-allocator callbacks to runtime `WalletOutputAllocator`.
#[derive(uniffi::Object)]
pub struct WalletOutputAllocatorLink {
    inner: Arc<dyn WalletAbiOutputAllocatorCallbacks>,
}

#[uniffi::export]
impl WalletOutputAllocatorLink {
    /// Create a wallet output-allocator bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiOutputAllocatorCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

impl WalletOutputAllocator for WalletOutputAllocatorLink {
    type Error = WalletOutputAllocatorLinkError;

    fn get_wallet_output_template(
        &self,
        session: &lwk_simplicity::wallet_abi::WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.inner
            .get_wallet_output_template(session_from_runtime(session), request.into())
            .map(template_from_binding)
            .map_err(|error| WalletOutputAllocatorLinkError::Foreign(format!("{error:?}")))
    }
}

fn template_from_binding(template: WalletAbiWalletOutputTemplate) -> WalletOutputTemplate {
    WalletOutputTemplate {
        script_pubkey: template.script_pubkey.as_ref().into(),
        blinding_pubkey: template
            .blinding_pubkey
            .map(|key| elements::secp256k1_zkp::PublicKey::from_slice(&key.to_bytes()))
            .transpose()
            .expect("binding public key must be a valid blinding public key"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{ExternalUtxo, Network, OutPoint, PublicKey, Script, TxOut, TxOutSecrets};
    use elements::Txid;
    use std::str::FromStr;

    struct TestOutputAllocatorCallbacks {
        template: WalletAbiWalletOutputTemplate,
    }

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            session: WalletAbiRequestSession,
            request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            assert_eq!(session.session_id, "session-42");
            assert_eq!(session.network, Network::regtest_default());
            assert_eq!(session.spendable_utxos.len(), 1);
            assert_eq!(request.role, crate::WalletAbiWalletOutputRole::Change);
            assert_eq!(request.ordinal, 3);
            assert_eq!(
                request.asset_id,
                Some(Network::regtest_default().policy_asset())
            );
            Ok(self.template.clone())
        }
    }

    #[test]
    fn wallet_output_allocator_link_adapts_foreign_callbacks() {
        let network = Network::regtest_default();
        let txid =
            Txid::from_str("3ac4f7d2d18e12256b4372d7947bf1df5cc640860cd63558e29cb2ec29319631")
                .expect("txid");
        let outpoint = OutPoint::from_parts(&txid.into(), 1);
        let txout = TxOut::from_explicit(&Script::empty(), network.policy_asset(), 5_000);
        let secrets = TxOutSecrets::from_explicit(network.policy_asset(), 5_000);
        let utxo = ExternalUtxo::from_unchecked_data(&outpoint, &txout, &secrets, 136);
        let script = Script::new(
            &"0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
                .parse()
                .expect("hex"),
        )
        .expect("script");
        let blinding_pubkey = PublicKey::from_string(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("blinding pubkey");
        let callbacks = Arc::new(TestOutputAllocatorCallbacks {
            template: WalletAbiWalletOutputTemplate {
                script_pubkey: script.clone(),
                blinding_pubkey: Some(blinding_pubkey.clone()),
            },
        });
        let link = WalletOutputAllocatorLink::new(callbacks);
        let session = lwk_simplicity::wallet_abi::WalletRequestSession {
            session_id: "session-42".to_string(),
            network: network.as_ref().into(),
            spendable_utxos: Arc::from(vec![utxo.as_ref().into()]),
        };

        let template = link
            .get_wallet_output_template(
                &session,
                &lwk_simplicity::wallet_abi::WalletOutputRequest::Change {
                    index: 3,
                    asset_id: network.policy_asset().into(),
                },
            )
            .expect("template");

        assert_eq!(
            template.script_pubkey,
            elements::Script::from(script.as_ref())
        );
        assert_eq!(
            template.blinding_pubkey.expect("blinding pubkey"),
            elements::secp256k1_zkp::PublicKey::from_slice(&blinding_pubkey.to_bytes())
                .expect("zkp pubkey")
        );
    }
}
