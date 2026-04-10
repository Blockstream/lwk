use std::sync::Arc;

use crate::{
    WalletBroadcasterLink, WalletOutputAllocatorLink, WalletPrevoutResolverLink,
    WalletReceiveAddressProviderLink, WalletSessionFactoryLink,
};

/// Convenience bundle of split wallet callback links used by the provider bridge.
#[derive(uniffi::Object)]
pub struct WalletRuntimeDepsLink {
    pub(crate) session_factory: Arc<WalletSessionFactoryLink>,
    pub(crate) output_allocator: Arc<WalletOutputAllocatorLink>,
    pub(crate) prevout_resolver: Arc<WalletPrevoutResolverLink>,
    pub(crate) broadcaster: Arc<WalletBroadcasterLink>,
    pub(crate) receive_address_provider: Arc<WalletReceiveAddressProviderLink>,
}

#[uniffi::export]
impl WalletRuntimeDepsLink {
    /// Create a wallet runtime dependency bundle from split wallet callback links.
    #[uniffi::constructor]
    pub fn new(
        session_factory: Arc<WalletSessionFactoryLink>,
        output_allocator: Arc<WalletOutputAllocatorLink>,
        prevout_resolver: Arc<WalletPrevoutResolverLink>,
        broadcaster: Arc<WalletBroadcasterLink>,
        receive_address_provider: Arc<WalletReceiveAddressProviderLink>,
    ) -> Self {
        Self {
            session_factory,
            output_allocator,
            prevout_resolver,
            broadcaster,
            receive_address_provider,
        }
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
        WalletAbiRequestSession, WalletAbiSessionFactoryCallbacks, WalletAbiWalletOutputRequest,
        WalletAbiWalletOutputTemplate,
    };

    struct TestSessionFactoryCallbacks;

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            unreachable!("not used in constructor test")
        }
    }

    struct TestOutputAllocatorCallbacks;

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            _session: WalletAbiRequestSession,
            _request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            unreachable!("not used in constructor test")
        }
    }

    struct TestPrevoutResolverCallbacks;

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<crate::WalletAbiBip32DerivationPair>, LwkError> {
            unreachable!("not used in constructor test")
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            unreachable!("not used in constructor test")
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            unreachable!("not used in constructor test")
        }
    }

    struct TestBroadcasterCallbacks;

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            unreachable!("not used in constructor test")
        }
    }

    struct TestReceiveAddressProviderCallbacks;

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            unreachable!("not used in constructor test")
        }
    }

    #[test]
    fn wallet_runtime_deps_link_keeps_all_split_roles() {
        let session_factory = Arc::new(WalletSessionFactoryLink::new(Arc::new(
            TestSessionFactoryCallbacks,
        )));
        let output_allocator = Arc::new(WalletOutputAllocatorLink::new(Arc::new(
            TestOutputAllocatorCallbacks,
        )));
        let prevout_resolver = Arc::new(WalletPrevoutResolverLink::new(Arc::new(
            TestPrevoutResolverCallbacks,
        )));
        let broadcaster = Arc::new(WalletBroadcasterLink::new(Arc::new(
            TestBroadcasterCallbacks,
        )));
        let receive_address_provider = Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
            TestReceiveAddressProviderCallbacks,
        )));
        let runtime_deps = WalletRuntimeDepsLink::new(
            session_factory.clone(),
            output_allocator.clone(),
            prevout_resolver.clone(),
            broadcaster.clone(),
            receive_address_provider.clone(),
        );

        assert!(Arc::ptr_eq(&runtime_deps.session_factory, &session_factory));
        assert!(Arc::ptr_eq(
            &runtime_deps.output_allocator,
            &output_allocator
        ));
        assert!(Arc::ptr_eq(
            &runtime_deps.prevout_resolver,
            &prevout_resolver
        ));
        assert!(Arc::ptr_eq(&runtime_deps.broadcaster, &broadcaster));
        assert!(Arc::ptr_eq(
            &runtime_deps.receive_address_provider,
            &receive_address_provider
        ));
    }
}
