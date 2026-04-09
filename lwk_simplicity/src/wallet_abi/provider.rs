use crate::error::WalletAbiError;
use crate::wallet_abi::schema::runtime_deps::SplitWalletProvider;
use crate::wallet_abi::schema::{KeyStoreMeta, WalletRuntimeDeps};

use lwk_wollet::secp256k1::XOnlyPublicKey;

/// Checked-in wallet-abi provider façade built on top of the runtime engine.
pub struct WalletAbiProvider<
    Signer,
    SessionFactory,
    PrevoutResolver,
    OutputAllocator,
    Broadcaster,
    ReceiveAddressProvider,
> {
    signer_meta: Signer,
    wallet_deps: WalletRuntimeDeps<
        SessionFactory,
        SplitWalletProvider<PrevoutResolver, OutputAllocator, Broadcaster>,
    >,
    receive_address_provider: ReceiveAddressProvider,
}

/// Constructor helper for assembling a source-owned wallet-abi provider façade.
pub struct WalletAbiProviderBuilder<
    Signer,
    SessionFactory,
    PrevoutResolver,
    OutputAllocator,
    Broadcaster,
    ReceiveAddressProvider,
> {
    signer_meta: Signer,
    session_factory: SessionFactory,
    prevout_resolver: PrevoutResolver,
    output_allocator: OutputAllocator,
    broadcaster: Broadcaster,
    receive_address_provider: ReceiveAddressProvider,
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProviderBuilder<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
{
    /// Capture the split provider roles and signer/session dependencies for one provider instance.
    pub fn new(
        signer_meta: Signer,
        session_factory: SessionFactory,
        prevout_resolver: PrevoutResolver,
        output_allocator: OutputAllocator,
        broadcaster: Broadcaster,
        receive_address_provider: ReceiveAddressProvider,
    ) -> Self {
        Self {
            signer_meta,
            session_factory,
            prevout_resolver,
            output_allocator,
            broadcaster,
            receive_address_provider,
        }
    }

    /// Assemble the provider façade from the captured runtime dependencies.
    pub fn build(
        self,
    ) -> WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    > {
        WalletAbiProvider {
            signer_meta: self.signer_meta,
            wallet_deps: WalletRuntimeDeps::new(
                self.session_factory,
                SplitWalletProvider::new(
                    self.prevout_resolver,
                    self.output_allocator,
                    self.broadcaster,
                ),
            ),
            receive_address_provider: self.receive_address_provider,
        }
    }
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
where
    Signer: KeyStoreMeta,
    WalletAbiError: From<Signer::Error>,
{
    /// Return the signer x-only public key exposed at provider connect time.
    pub fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, WalletAbiError> {
        self.signer_meta
            .get_raw_signing_x_only_pubkey()
            .map_err(WalletAbiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiProviderBuilder, XOnlyPublicKey};
    use crate::error::WalletAbiError;
    use crate::wallet_abi::schema::KeyStoreMeta;

    use lwk_wollet::elements::pset::PartiallySignedTransaction;
    use lwk_wollet::secp256k1::schnorr::Signature;
    use lwk_wollet::secp256k1::{Message, XOnlyPublicKey as SecpXOnlyPublicKey};

    use std::str::FromStr;

    #[derive(Debug, thiserror::Error)]
    #[error("test provider error")]
    struct TestProviderError;

    impl From<TestProviderError> for WalletAbiError {
        fn from(error: TestProviderError) -> Self {
            WalletAbiError::InvalidRequest(error.to_string())
        }
    }

    struct TestSigner;
    struct TestSessionFactory;
    struct TestPrevoutResolver;
    struct TestOutputAllocator;
    struct TestBroadcaster;
    struct TestReceiveAddressProvider;

    impl KeyStoreMeta for TestSigner {
        type Error = TestProviderError;

        fn get_raw_signing_x_only_pubkey(&self) -> Result<SecpXOnlyPublicKey, Self::Error> {
            SecpXOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|_| TestProviderError)
        }

        fn sign_pst(&self, _pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
            Ok(())
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: SecpXOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            Err(TestProviderError)
        }
    }

    #[test]
    fn provider_xonly_getter() {
        let provider = WalletAbiProviderBuilder::new(
            TestSigner,
            TestSessionFactory,
            TestPrevoutResolver,
            TestOutputAllocator,
            TestBroadcaster,
            TestReceiveAddressProvider,
        )
        .build();

        assert_eq!(
            provider
                .get_raw_signing_x_only_pubkey()
                .expect("xonly pubkey"),
            XOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .expect("valid xonly pubkey"),
        );
    }
}
