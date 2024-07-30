mod apdu;
mod client;
mod command;
mod error;
mod interpreter;
mod merkle;
mod psbt;
mod transport;
mod wallet;

#[cfg(feature = "test_emulator")]
mod ledger_emulator;

#[cfg(feature = "test_emulator")]
pub use ledger_emulator::TestLedgerEmulator;

// Adapted from
// https://github.com/LedgerHQ/app-bitcoin-new/tree/master/bitcoin_client_rs
pub use client::LiquidClient;
pub use transport::TransportTcp;
pub use wallet::{AddressType, Version, WalletPolicy, WalletPubKey};

use elements_miniscript::confidential::slip77;
use elements_miniscript::elements::bitcoin::bip32::{
    ChildNumber, DerivationPath, Fingerprint, Xpub,
};
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use elements_miniscript::elements::{
    opcodes::{
        all::{OP_CHECKMULTISIG, OP_PUSHNUM_1, OP_PUSHNUM_16},
        All,
    },
    script::Instruction,
    Script,
};

use lwk_common::Signer;

#[derive(Debug)]
pub struct Ledger {
    /// Ledger Liquid Client
    pub client: LiquidClient<TransportTcp>,
}

impl Ledger {
    pub fn new(port: u16) -> Self {
        let client = LiquidClient::new(TransportTcp::new(port).expect("TODO"));
        Self { client }
    }
}

pub type Error = error::LiquidClientError<TransportTcp>;

impl Signer for &Ledger {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> std::result::Result<u32, Self::Error> {
        // Set the default values some fields that Ledger requires
        if pset.global.tx_data.fallback_locktime.is_none() {
            pset.global.tx_data.fallback_locktime =
                Some(elements_miniscript::elements::LockTime::ZERO);
        }
        for input in pset.inputs_mut() {
            if input.sequence.is_none() {
                input.sequence = Some(elements_miniscript::elements::Sequence::default());
            }
        }

        let mut wallets = vec![];
        let mut n_sigs = 0;
        let master_fp = self.fingerprint()?;

        // Figure out which wallets are signing
        for input in pset.inputs() {
            let script_pubkey = &input.witness_utxo.as_ref().expect("FIXME").script_pubkey;
            let is_p2wpkh = script_pubkey.is_v0_p2wpkh();
            let is_p2shwpkh = script_pubkey.is_p2sh()
                && input
                    .redeem_script
                    .as_ref()
                    .map(|x| x.is_v0_p2wpkh())
                    .unwrap_or(false);
            let desc = if is_p2wpkh {
                "wpkh(@0)"
            } else if is_p2shwpkh {
                "sh(wpkh(@0))"
            } else {
                ""
            };
            if desc.is_empty() {
                // TODO: add support for other scripts
                continue;
            }
            for (_pubkey, (fp, path)) in input.bip32_derivation.iter() {
                if fp == &master_fp {
                    // TODO: check path
                    // path has len 3
                    // path has all hardened
                    // path has purpose matching address type
                    // path has correct coin type
                    let mut v: Vec<ChildNumber> = path.clone().into();
                    v.truncate(3);
                    let path: DerivationPath = v.into();

                    // Do we care about the descriptor blinding key here?
                    let name = "todo".to_string();
                    let version = Version::V1;
                    // TODO: cache xpubs
                    let xpub = self
                        .client
                        .get_extended_pubkey(&path, false)
                        .expect("FIXME");
                    let mut key = WalletPubKey::from(((*fp, path.clone()), xpub));
                    key.multipath = Some("/**".to_string());
                    let keys = vec![key];
                    let wallet_policy = WalletPolicy::new(name, version, desc.to_string(), keys);
                    wallets.push(wallet_policy);
                }
            }
        }

        // For each wallet, sign
        for wallet_policy in wallets {
            let partial_sigs = self
                .client
                .sign_psbt(
                    pset,
                    &wallet_policy,
                    None, // hmac
                )
                .expect("FIXME");
            n_sigs += partial_sigs.len();

            // Add sigs to pset
            for (input_idx, sig) in partial_sigs {
                let input = &mut pset.inputs_mut()[input_idx];
                // FIXME: how to associate a signature to the corresponding pubkey?
                let public_key = *input.bip32_derivation.keys().nth(0).expect("FIXME");
                input.partial_sigs.insert(public_key, sig.to_vec());
            }
        }

        Ok(n_sigs as u32)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        let r = self.client.get_extended_pubkey(path, false).expect("FIXME");
        Ok(r)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        let r = self.client.get_master_blinding_key().expect("FIXME");
        Ok(r)
    }

    fn fingerprint(&self) -> std::result::Result<Fingerprint, Self::Error> {
        let r = self.client.get_master_fingerprint().expect("FIXME");
        Ok(r)
    }
}

impl Signer for Ledger {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> std::result::Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }

    fn fingerprint(&self) -> std::result::Result<Fingerprint, Self::Error> {
        Signer::fingerprint(&self)
    }
}

// duplicated from Jade
// taken and adapted from:
// https://github.com/rust-bitcoin/rust-bitcoin/blob/37daf4620c71dc9332c3e08885cf9de696204bca/bitcoin/src/blockdata/script/borrowed.rs#L266
// TODO remove once it's released
fn is_multisig(script: &Script) -> bool {
    fn decode_pushnum(op: All) -> Option<u8> {
        let start: u8 = OP_PUSHNUM_1.into_u8();
        let end: u8 = OP_PUSHNUM_16.into_u8();
        if start < op.into_u8() && end >= op.into_u8() {
            Some(op.into_u8() - start + 1)
        } else {
            None
        }
    }

    let required_sigs;

    let mut instructions = script.instructions();
    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if let Some(pushnum) = decode_pushnum(op) {
            required_sigs = pushnum;
        } else {
            return false;
        }
    } else {
        return false;
    }

    let mut num_pubkeys: u8 = 0;
    while let Some(Ok(instruction)) = instructions.next() {
        match instruction {
            Instruction::PushBytes(_) => {
                num_pubkeys += 1;
            }
            Instruction::Op(op) => {
                if let Some(pushnum) = decode_pushnum(op) {
                    if pushnum != num_pubkeys {
                        return false;
                    }
                }
                break;
            }
        }
    }

    if required_sigs > num_pubkeys {
        return false;
    }

    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if op != OP_CHECKMULTISIG {
            return false;
        }
    } else {
        return false;
    }

    instructions.next().is_none()
}
