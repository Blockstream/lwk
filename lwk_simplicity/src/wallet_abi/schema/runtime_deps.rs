use std::future::Future;
use std::sync::Arc;

use lwk_wollet::bitcoin::bip32::KeySource;
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{AssetId, OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
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
    pub network: lwk_common::Network,
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

/// Resolve prevouts, output requests, wallet-owned unblinding, BIP32 metadata, and tx broadcast.
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
