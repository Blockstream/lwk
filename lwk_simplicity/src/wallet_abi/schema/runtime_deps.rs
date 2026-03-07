use std::future::Future;
use std::sync::Arc;

use lwk_common::Bip;
use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint};
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
use lwk_wollet::WalletTxOut;

/// Runtime-provided signer capabilities required by wallet-abi transaction resolution.
pub trait SignerMeta {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Return active network used for request/network consistency checks.
    fn get_network(&self) -> lwk_common::Network;

    /// Return signer receive address used for default wallet outputs/change.
    ///
    /// Runtime uses this address when it needs a wallet-owned fallback script destination
    /// (for example deterministic change paths driven by signer-owned material).
    ///
    /// Contract requirements:
    /// - address MUST belong to the same signer context as derivation/pubkey methods below
    /// - address MUST match `get_network()`
    fn get_signer_receive_address(&self) -> Result<Address, Self::Error>;

    /// Return signer master fingerprint for BIP32 origin metadata.
    ///
    /// This fingerprint is paired with derivation paths produced by
    /// [`Self::get_derivation_path`] and written into `bip32_derivation` maps.
    ///
    /// Contract requirements:
    /// - MUST identify the same root key lineage used by pubkeys from
    ///   [`Self::get_pubkey_by_derivation_path`]
    fn fingerprint(&self) -> Fingerprint;

    /// Return base derivation path for the selected BIP branch.
    ///
    /// Runtime appends wallet-specific child segments (i.e. external/internal branch and
    /// wildcard index) to this base path when constructing per-input key origins.
    ///
    /// Contract requirements:
    /// - returned path MUST be deterministic for `(signer, bip)`
    /// - returned path MUST be compatible with [`Self::get_pubkey_by_derivation_path`]
    fn get_derivation_path(&self, bip: Bip) -> DerivationPath;

    /// Return public key for a derived BIP32 path.
    ///
    /// This is the pubkey counterpart of [`Self::get_derivation_path`] + runtime-appended
    /// children: runtime records `(fingerprint, derivation_path, pubkey)` triplets in PSET input
    /// origin metadata.
    ///
    /// Contract requirements:
    /// - for the same `derivation_path`, return value MUST be stable within one request lifecycle
    /// - pubkey MUST correspond to the signer key lineage identified by [`Self::fingerprint`]
    fn get_pubkey_by_derivation_path(
        &self,
        derivation_path: &DerivationPath,
    ) -> Result<PublicKey, Self::Error>;

    /// Return signer x-only public key used by runtime witness directives.
    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error>;

    /// Unblind one transaction output and return explicit values and blinding factors.
    ///
    /// Runtime calls this for inputs that request wallet-managed unblinding.
    ///
    /// Returns `TxOutSecrets` carrying `(asset, value, asset_bf, value_bf)`
    ///
    /// Failure contract:
    /// - return `Err` when output cannot be unblinded with signer-controlled blinding material
    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error>;

    /// Sign wallet-finalized PSET inputs.
    ///
    /// Runtime currently finalizes non-taproot wallet paths using the miniscript stack and
    /// passes `BlockHash::all_zeros()` in finalization checks as expected by current signer flow.
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

/// Runtime-provided wallet/backend capabilities required by transaction resolution.
pub trait WalletMeta {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fetch previous output by outpoint.
    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_;

    /// Broadcast a finalized transaction and return the backend-reported txid.
    fn broadcast_transaction(
        &self,
        tx: Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_;

    /// Return spendable wallet UTXOs used as the candidate input pool.
    ///
    /// Contract requirements:
    /// - entries MUST be unspent
    /// - entries MUST include valid unblinded secrets
    /// - membership SHOULD remain deterministic for one runtime request lifecycle
    fn get_spendable_utxos(
        &self,
    ) -> impl Future<Output = Result<Vec<WalletTxOut>, Self::Error>> + Send + '_;
}

// The provider stores foreign callback bridges behind `Arc`, so runtime trait
// calls need to transparently forward through shared ownership.
impl<T> SignerMeta for Arc<T>
where
    T: SignerMeta + ?Sized,
{
    type Error = T::Error;

    fn get_network(&self) -> lwk_common::Network {
        self.as_ref().get_network()
    }

    fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
        self.as_ref().get_signer_receive_address()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.as_ref().fingerprint()
    }

    fn get_derivation_path(&self, bip: Bip) -> DerivationPath {
        self.as_ref().get_derivation_path(bip)
    }

    fn get_pubkey_by_derivation_path(
        &self,
        derivation_path: &DerivationPath,
    ) -> Result<PublicKey, Self::Error> {
        self.as_ref().get_pubkey_by_derivation_path(derivation_path)
    }

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        self.as_ref().get_raw_signing_x_only_pubkey()
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        self.as_ref().unblind(tx_out)
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

// Mirror the signer forwarding above so one provider instance can own both
// callback bridges without rebuilding per request.
impl<T> WalletMeta for Arc<T>
where
    T: WalletMeta + ?Sized,
{
    type Error = T::Error;

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        self.as_ref().get_tx_out(outpoint)
    }

    fn broadcast_transaction(
        &self,
        tx: Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        self.as_ref().broadcast_transaction(tx)
    }

    fn get_spendable_utxos(
        &self,
    ) -> impl Future<Output = Result<Vec<WalletTxOut>, Self::Error>> + Send + '_ {
        self.as_ref().get_spendable_utxos()
    }
}
