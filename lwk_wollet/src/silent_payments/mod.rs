//! Silent payments on Liquid, an implementation of [BIP352] adapted to confidential
//! transactions.
//!
//! A silent payment address is a static address that can be published without giving up
//! privacy: the sender derives a fresh output from the address and from the inputs it is
//! spending, and only the receiver can recognize it. Nothing is added to the transaction, so
//! the receiver has to scan the chain looking for outputs paying to it.
//!
//! # Differences from BIP352
//!
//! The key derivation is unchanged, what Liquid adds is blinding. Outputs are confidential, so
//! the sender must blind them to a key the receiver can compute back: that key is derived from
//! the same shared secret used for the output key, with the tag
//! `Silent-Payment-Blinding-Key/1.0`.
//!
//! Addresses use the same encoding of BIP352 with a different human readable part, so that an
//! address of the wrong chain cannot be paid by mistake: `lqsp` for Liquid, `tlqsp` for Liquid
//! testnet and `elsp` for Elements regtest.
//!
//! Peg-in inputs never contribute to the shared secret, their spending data is on the Bitcoin
//! chain. Their outpoints are still considered when looking for the smallest one.
//!
//! # Receiving
//!
//! ```
//! # use lwk_wollet::silent_payments::{SilentPaymentKeys, SilentPaymentWollet};
//! # use lwk_wollet::elements::{Script, Transaction};
//! # use lwk_wollet::{Network, WolletBuilder};
//! # fn main() -> Result<(), lwk_wollet::Error> {
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let network = Network::TestnetLiquid;
//! let keys = SilentPaymentKeys::from_mnemonic(mnemonic, network, 0)?;
//! let mut wollet = SilentPaymentWollet::new(network, keys.scan_secret_key(), keys.spend_public_key());
//!
//! // give this address out, it can be reused without linking the payments
//! println!("{}", wollet.address());
//! # Ok(())
//! # }
//!
//! // for every transaction of the chain, together with the scripts of the outputs it spends
//! fn scan(
//!     wollet: &mut SilentPaymentWollet,
//!     tx: &Transaction,
//!     prevout_script_pubkeys: &[Script],
//! ) -> Result<(), lwk_wollet::Error> {
//!     wollet.scan_transaction(tx, prevout_script_pubkeys)?;
//!
//!     // the discovered outputs can be tracked by a regular watch only wallet
//!     if !wollet.is_empty() {
//!         let wollet = WolletBuilder::new(wollet.network(), wollet.wollet_descriptor()?).build()?;
//!     }
//!     Ok(())
//! }
//! ```
//!
//! [BIP352]: https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki

use elements::OutPoint;

mod address;
mod hashes;
mod inputs;
mod keys;
mod scan;
mod send;
mod wollet;

#[cfg(test)]
mod test_vectors;

pub use address::{SilentPaymentAddress, SilentPaymentNetwork};
pub use inputs::{transaction_inputs, tweak_data, tweak_data_from_tx, SilentPaymentInput};
pub use keys::SilentPaymentKeys;
pub use scan::{SilentPaymentScanner, SilentPaymentTxOut, CHANGE_LABEL, K_MAX};
pub use send::{derive_outputs, SilentPaymentOutput};
pub use wollet::SilentPaymentWollet;

/// Errors of the silent payments module
#[derive(thiserror::Error, Debug)]
pub enum SilentPaymentError {
    /// The address is not a valid bech32m string
    #[error("Invalid silent payment address: {0}")]
    AddressEncoding(String),

    /// The address has a version this implementation cannot handle
    #[error("Unsupported silent payment address version '{0}'")]
    AddressVersion(char),

    /// The address does not contain two public keys
    #[error("Invalid silent payment address payload length: {0}")]
    AddressLength(usize),

    /// The address is not for a Liquid network
    #[error("Unknown silent payment address prefix '{0}'")]
    AddressHrp(String),

    /// The scripts of the spent outputs do not match the transaction inputs
    #[error("The transaction has {inputs} inputs but {prevouts} previous outputs were given")]
    PrevoutsMismatch {
        /// Number of inputs of the transaction
        inputs: usize,
        /// Number of previous outputs given
        prevouts: usize,
    },

    /// A transaction always has at least one input
    #[error("The transaction has no inputs")]
    NoInputs,

    /// None of the inputs can contribute to the shared secret, so no output can be derived
    #[error("None of the transaction inputs is eligible for silent payments")]
    NoEligibleInputs,

    /// The sum of the input public keys is the point at infinity, the transaction cannot pay
    /// to a silent payment address
    #[error("The inputs of the transaction sum up to the point at infinity")]
    InputsSumToInfinity,

    /// An input eligible for silent payments was given without its secret key
    #[error("No secret key was given for input {0}")]
    MissingInputSecretKey(OutPoint),

    /// The secret key given for an input does not match the public key of that input
    #[error("The secret key given for input {0} does not match its public key")]
    WrongInputSecretKey(OutPoint),

    /// The input cannot contribute its public key, either because the output it spends is not
    /// one of the eligible types or because the key given does not match that output
    #[error("The input {0} cannot contribute its public key")]
    IneligibleInput(OutPoint),

    /// No recipient was given
    #[error("No recipient was given")]
    NoRecipients,

    /// A transaction cannot pay more than [`K_MAX`] outputs to the same scan key
    #[error("More than {K_MAX} outputs pay to the same scan key")]
    TooManyOutputsPerScanKey,

    /// The recipients are not all on the same network
    #[error("The recipients are not all on the same network")]
    MixedNetworks,

    /// Happens with negligible probability
    #[error("The input hash is not a valid secret key")]
    InvalidInputHash,

    /// Happens with negligible probability
    #[error("The label hash is not a valid secret key")]
    InvalidLabelHash,

    /// Happens with negligible probability
    #[error("The shared secret hash is not a valid secret key")]
    InvalidSharedSecretHash,

    /// Happens with negligible probability
    #[error("The blinding key hash is not a valid secret key")]
    InvalidBlindingHash,

    /// Error from the secp256k1 library
    #[error(transparent)]
    Secp256k1(#[from] crate::secp256k1::Error),
}
