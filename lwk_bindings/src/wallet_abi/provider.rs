use std::sync::Arc;

use crate::{LwkError, SignerMetaLink, WalletRuntimeDepsLink, XOnlyPublicKey};
use lwk_simplicity::wallet_abi::KeyStoreMeta;

/// Source-owned bindings wrapper for the checked-in Wallet ABI provider facade.
#[derive(uniffi::Object)]
pub struct WalletAbiProvider {
    signer: Arc<SignerMetaLink>,
    wallet: Arc<WalletRuntimeDepsLink>,
}

#[uniffi::export]
impl WalletAbiProvider {
    /// Create a provider object from a signer bridge and split wallet runtime dependencies.
    #[uniffi::constructor]
    pub fn new(signer: Arc<SignerMetaLink>, wallet: Arc<WalletRuntimeDepsLink>) -> Self {
        Self { signer, wallet }
    }

    /// Return the signer x-only public key exposed at provider connect time.
    pub fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
        Ok(Arc::new(
            self.signer
                .get_raw_signing_x_only_pubkey()
                .map_err(|error| LwkError::from(format!("{error}")))?
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        Address, LwkError, OutPoint, Transaction, TxOut, TxOutSecrets, Txid,
        WalletAbiBroadcasterCallbacks, WalletAbiOutputAllocatorCallbacks,
        WalletAbiPrevoutResolverCallbacks, WalletAbiReceiveAddressProviderCallbacks,
        WalletAbiRequestSession, WalletAbiSessionFactoryCallbacks, WalletAbiSignerCallbacks,
        WalletAbiWalletOutputRequest, WalletAbiWalletOutputTemplate, Pset,
        WalletBroadcasterLink, WalletOutputAllocatorLink, WalletPrevoutResolverLink,
        WalletReceiveAddressProviderLink, WalletSessionFactoryLink,
    };
    use elements::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};

    struct TestSignerCallbacks {
        keypair: Keypair,
    }

    impl WalletAbiSignerCallbacks for TestSignerCallbacks {
        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            Ok(XOnlyPublicKey::from_keypair(&self.keypair))
        }

        fn sign_pst(&self, _pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            unreachable!("not used in provider xonly test")
        }

        fn sign_schnorr(&self, _message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestSessionFactoryCallbacks;

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestPrevoutResolverCallbacks;

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<crate::WalletAbiBip32DerivationPair>, LwkError> {
            unreachable!("not used in provider xonly test")
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            unreachable!("not used in provider xonly test")
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestOutputAllocatorCallbacks;

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            _session: WalletAbiRequestSession,
            _request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestBroadcasterCallbacks;

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestReceiveAddressProviderCallbacks;

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    #[test]
    fn wallet_abi_provider_get_raw_signing_x_only_pubkey() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x11; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let expected = keypair.x_only_public_key().0.to_string();
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks { keypair }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        assert_eq!(
            provider
                .get_raw_signing_x_only_pubkey()
                .expect("x-only public key")
                .to_string(),
            expected
        );
    }
}
