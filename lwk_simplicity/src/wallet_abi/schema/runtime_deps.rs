use std::future::Future;
use std::sync::Arc;

use lwk_wollet::bitcoin::bip32::KeySource;
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{
    Address, AssetId, OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid,
};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
use lwk_wollet::{BlindingPublicKey, ExternalUtxo};

/// Wallet-owned output destination data selected by wallet policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletOutputTemplate {
    /// Output script pubkey selected by wallet policy for this role/request.
    pub script_pubkey: Script,
    /// Optional blinding public key for confidential outputs.
    pub blinding_pubkey: Option<BlindingPublicKey>,
}

/// Request-scoped wallet session snapshot.
///
/// Runtime should open this once per request and should reuse it across fee estimation and final
/// build.
#[derive(Debug, Clone)]
pub struct WalletRequestSession {
    /// Opaque wallet-owned request/session correlation identifier.
    pub session_id: String,
    /// Network for request validation and script derivation.
    pub network: lwk_wollet::ElementsNetwork,
    /// Deterministic wallet UTXO snapshot used for all input-selection work in this request.
    ///
    /// For a fixed request/session, this snapshot must stay stable across fee
    /// estimation and the final build.
    pub spendable_utxos: Arc<[ExternalUtxo]>,
}

/// Deterministic wallet output selection request.
///
/// For the same `(session, request)` pair, `get_wallet_output_template()` must return the same
/// template across fee estimation and final build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletOutputRequest {
    /// Wallet-owned receive output destination.
    Receive {
        /// Deterministic zero-based address index within this build pass.
        index: u32,
    },
    /// Runtime-added change output destination for one concrete asset.
    Change {
        /// Deterministic zero-based change index within this build pass.
        index: u32,
        /// Residual asset id being returned to the wallet.
        asset_id: AssetId,
    },
}

/// Runtime dependency bundle for wallet-owned behavior.
pub struct WalletRuntimeDeps<SessionFactory, WalletProvider> {
    /// Request-session factory used once per runtime request.
    pub session_factory: SessionFactory,
    /// Wallet provider that does:
    /// 1. Prevout resolution, wallet-owned metadata/unblinding.
    /// 2. Wallet-owned output allocator for receive/change destinations.
    /// 3. Transaction broadcaster.
    pub wallet_provider: WalletProvider,
}

impl<SessionFactory, WalletProvider> WalletRuntimeDeps<SessionFactory, WalletProvider> {
    /// Build a runtime dependency bundle from split wallet components.
    pub fn new(session_factory: SessionFactory, wallet_provider: WalletProvider) -> Self {
        Self {
            session_factory,
            wallet_provider,
        }
    }
}

/// Runtime-provided signer capabilities required by wallet-abi transaction resolution.
///
/// This trait is intentionally limited to signing and witness-related operations.
pub trait KeyStoreMeta {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return signer x-only public key to be used by runtime witness directives.
    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error>;

    /// Sign wallet-finalized PSET inputs.
    ///
    /// For the current signer flow, runtime should finalize non-taproot wallet paths using the
    /// miniscript stack and should pass `BlockHash::all_zeros()` in finalization checks.
    ///
    /// Mutation contract:
    /// - SHOULD add/update only signing/finalization material required for wallet-finalized inputs
    /// - SHOULD NOT rewrite unrelated global/input/output fields
    /// - SHOULD leave non-wallet/Simplicity-managed inputs untouched
    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error>;

    /// Create one Schnorr signature for a precomputed secp256k1 message digest.
    ///
    /// Message is already a 32-byte digest (`secp256k1::Message`), not arbitrary payload.
    ///
    /// Coupling contract:
    /// - signature MUST verify under `xonly_public_key`
    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: XOnlyPublicKey,
    ) -> Result<Signature, Self::Error>;
}

/// Open one request-scoped wallet session snapshot.
pub trait WalletSessionFactory {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Open a wallet request session for one runtime request lifecycle.
    fn open_wallet_request_session(
        &self,
    ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_;
}

/// Resolve wallet-owned prevouts, metadata, and unblinding for runtime input materialization.
pub trait WalletPrevoutResolver {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return the wallet-owned BIP32 origin metadata for the selected outpoint, if available.
    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error>;

    /// Unblind one transaction output using wallet-owned descriptor or blinding material.
    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error>;

    /// Fetch tx out structure by outpoint.
    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_;
}

/// Allocate deterministic wallet-owned output destinations for receive and change requests.
pub trait WalletOutputAllocator {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return the wallet-owned output template for a deterministic request selector.
    ///
    /// Stability contract:
    /// - for the same `(session, request)`, this must return the same template across fee
    ///   estimation and final build
    /// - change templates must include a blinding pubkey when runtime will blind them
    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error>;
}

/// Broadcast finalized transactions through the active wallet backend.
pub trait WalletBroadcaster {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Broadcast a finalized transaction and return the backend-reported txid.
    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_;
}

/// Provide the deterministic wallet receive address exposed at provider connect time.
pub trait WalletReceiveAddressProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return the active wallet receive address for the provider session.
    fn get_signer_receive_address(&self) -> Result<Address, Self::Error>;
}

/// Legacy aggregate runtime dependency trait preserved while the public wallet provider boundary
/// moves to split roles.
///
/// These traits use generic/static dispatch via `impl Future`; the `Arc<T>` forwarding impls
/// preserve shared ownership of concrete callback implementations, but they do not make the traits
/// object-safe.
pub trait WalletProviderMeta {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return the wallet-owned BIP32 origin metadata for the selected outpoint, if available.
    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error>;

    /// Unblind one transaction output using wallet-owned descriptor or blinding material.
    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error>;

    /// Fetch tx out structure by outpoint.
    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_;

    /// Return the wallet-owned output template for a deterministic request selector.
    ///
    /// Stability contract:
    /// - for the same `(session, request)`, this must return the same template across fee
    ///   estimation and final build
    /// - change templates must include a blinding pubkey when runtime will blind them
    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error>;

    /// Broadcast a finalized transaction and return the backend-reported txid.
    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_;
}

/// Adapter that composes split wallet roles into the legacy runtime aggregate.
pub(crate) struct SplitWalletProvider<PrevoutResolver, OutputAllocator, Broadcaster> {
    prevout_resolver: PrevoutResolver,
    output_allocator: OutputAllocator,
    broadcaster: Broadcaster,
}

impl<PrevoutResolver, OutputAllocator, Broadcaster>
    SplitWalletProvider<PrevoutResolver, OutputAllocator, Broadcaster>
{
    pub(crate) fn new(
        prevout_resolver: PrevoutResolver,
        output_allocator: OutputAllocator,
        broadcaster: Broadcaster,
    ) -> Self {
        Self {
            prevout_resolver,
            output_allocator,
            broadcaster,
        }
    }
}

impl<PrevoutResolver, OutputAllocator, Broadcaster> WalletProviderMeta
    for SplitWalletProvider<PrevoutResolver, OutputAllocator, Broadcaster>
where
    PrevoutResolver: WalletPrevoutResolver,
    OutputAllocator: WalletOutputAllocator<Error = PrevoutResolver::Error>,
    Broadcaster: WalletBroadcaster<Error = PrevoutResolver::Error>,
{
    type Error = PrevoutResolver::Error;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.prevout_resolver.get_bip32_derivation_pair(out_point)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        self.prevout_resolver.unblind(tx_out)
    }

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        self.prevout_resolver.get_tx_out(outpoint)
    }

    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.output_allocator
            .get_wallet_output_template(session, request)
    }

    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        self.broadcaster.broadcast_transaction(tx)
    }
}

// The provider stores foreign callback bridges behind `Arc`, so runtime trait
// calls need to transparently forward through shared ownership.
impl<T> KeyStoreMeta for Arc<T>
where
    T: KeyStoreMeta + ?Sized,
{
    type Error = T::Error;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        self.as_ref().get_raw_signing_x_only_pubkey()
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.as_ref().sign_pst(pst)
    }

    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: XOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        self.as_ref().sign_schnorr(message, xonly_public_key)
    }
}

impl<T> WalletSessionFactory for Arc<T>
where
    T: WalletSessionFactory + ?Sized,
{
    type Error = T::Error;

    fn open_wallet_request_session(
        &self,
    ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_ {
        self.as_ref().open_wallet_request_session()
    }
}

impl<T> WalletPrevoutResolver for Arc<T>
where
    T: WalletPrevoutResolver + ?Sized,
{
    type Error = T::Error;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.as_ref().get_bip32_derivation_pair(out_point)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        self.as_ref().unblind(tx_out)
    }

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        self.as_ref().get_tx_out(outpoint)
    }
}

impl<T> WalletOutputAllocator for Arc<T>
where
    T: WalletOutputAllocator + ?Sized,
{
    type Error = T::Error;

    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.as_ref().get_wallet_output_template(session, request)
    }
}

impl<T> WalletBroadcaster for Arc<T>
where
    T: WalletBroadcaster + ?Sized,
{
    type Error = T::Error;

    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        self.as_ref().broadcast_transaction(tx)
    }
}

impl<T> WalletReceiveAddressProvider for Arc<T>
where
    T: WalletReceiveAddressProvider + ?Sized,
{
    type Error = T::Error;

    fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
        self.as_ref().get_signer_receive_address()
    }
}

impl<T> WalletProviderMeta for Arc<T>
where
    T: WalletProviderMeta + ?Sized,
{
    type Error = T::Error;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.as_ref().get_bip32_derivation_pair(out_point)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        self.as_ref().unblind(tx_out)
    }

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        self.as_ref().get_tx_out(outpoint)
    }

    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.as_ref().get_wallet_output_template(session, request)
    }

    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        self.as_ref().broadcast_transaction(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::future::Future;
    use std::pin::pin;
    use std::str::FromStr;
    use std::task::{Context, Poll, Waker};

    use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint};
    use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use lwk_wollet::elements::hashes::Hash;

    #[derive(Debug, thiserror::Error)]
    #[error("test wallet role error")]
    struct TestWalletRoleError;

    struct TestPrevoutResolver {
        derivation_pair: (PublicKey, KeySource),
        secrets: TxOutSecrets,
        tx_out: TxOut,
    }

    impl WalletPrevoutResolver for TestPrevoutResolver {
        type Error = TestWalletRoleError;

        fn get_bip32_derivation_pair(
            &self,
            _out_point: &OutPoint,
        ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
            Ok(self.secrets)
        }

        fn get_tx_out(
            &self,
            _outpoint: OutPoint,
        ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
            async move { Ok(self.tx_out.clone()) }
        }
    }

    struct TestOutputAllocator {
        template: WalletOutputTemplate,
    }

    impl WalletOutputAllocator for TestOutputAllocator {
        type Error = TestWalletRoleError;

        fn get_wallet_output_template(
            &self,
            _session: &WalletRequestSession,
            _request: &WalletOutputRequest,
        ) -> Result<WalletOutputTemplate, Self::Error> {
            Ok(self.template.clone())
        }
    }

    struct TestBroadcaster {
        txid: Txid,
    }

    impl WalletBroadcaster for TestBroadcaster {
        type Error = TestWalletRoleError;

        fn broadcast_transaction(
            &self,
            _tx: &Transaction,
        ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
            async move { Ok(self.txid) }
        }
    }

    #[test]
    fn split_wallet_provider_forwards_roles() {
        let script_pubkey = Script::new();
        let tx_out = TxOut::default();
        let tx = Transaction {
            version: 2,
            lock_time: lwk_wollet::elements::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        let out_point = OutPoint {
            txid: Txid::all_zeros(),
            vout: 3,
        };
        let derivation_pair = (
            PublicKey::from_str(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .expect("valid pubkey"),
            (
                Fingerprint::from_str("01020304").expect("valid fingerprint"),
                DerivationPath::from_str("m/84h/1h/0h/0/0").expect("valid derivation path"),
            ),
        );
        let secrets = TxOutSecrets::new(
            lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset(),
            AssetBlindingFactor::zero(),
            42,
            ValueBlindingFactor::zero(),
        );
        let template = WalletOutputTemplate {
            script_pubkey,
            blinding_pubkey: None,
        };
        let provider = SplitWalletProvider::new(
            TestPrevoutResolver {
                derivation_pair: derivation_pair.clone(),
                secrets,
                tx_out: tx_out.clone(),
            },
            TestOutputAllocator {
                template: template.clone(),
            },
            TestBroadcaster { txid: tx.txid() },
        );

        assert_eq!(
            provider
                .get_bip32_derivation_pair(&out_point)
                .expect("derivation pair"),
            Some(derivation_pair)
        );
        assert_eq!(provider.unblind(&tx_out).expect("unblind"), secrets);
        assert_eq!(
            ready(provider.get_tx_out(out_point)).expect("tx out"),
            tx_out
        );
        assert_eq!(
            provider
                .get_wallet_output_template(
                    &WalletRequestSession {
                        session_id: "session-1".to_string(),
                        network: lwk_wollet::ElementsNetwork::LiquidTestnet,
                        spendable_utxos: Arc::from(Vec::<ExternalUtxo>::new()),
                    },
                    &WalletOutputRequest::Receive { index: 0 }
                )
                .expect("wallet output template"),
            template
        );
        assert_eq!(
            ready(provider.broadcast_transaction(&tx)).expect("broadcast"),
            tx.txid()
        );
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut future = pin!(future);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }
}
