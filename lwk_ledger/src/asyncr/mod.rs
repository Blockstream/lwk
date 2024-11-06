pub use crate::psbt::PartialSignature;
pub use client::{LiquidClient, Transport};
use elements_miniscript::{
    bitcoin::bip32::{ChildNumber, DerivationPath, Fingerprint, Xpub},
    elements::pset::PartiallySignedTransaction,
    slip77,
};
pub use transport_tcp::TransportTcp;

use crate::{parse_multisig, AddressType, Error, Version, WalletPolicy, WalletPubKey};

mod client;
mod transport_tcp;

#[derive(Debug)]
pub struct Ledger<T: Transport> {
    /// Ledger Liquid Client
    pub client: LiquidClient<T>,
}

impl Ledger<TransportTcp> {
    pub fn new(port: u16) -> Self {
        let client = LiquidClient::new(TransportTcp::new(port).expect("TODO"));
        Self { client }
    }
}

/// TODO Should implement Signer, but here we are async. Make async signer and impl here and for jade
impl Ledger<TransportTcp> {
    pub async fn sign(
        &self,
        pset: &mut PartiallySignedTransaction,
    ) -> std::result::Result<u32, Error> {
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

        // Use a map to avoid inserting a wallet twice
        let mut wallets = std::collections::HashMap::<String, WalletPolicy>::new();
        let mut n_sigs = 0;
        let master_fp = self.fingerprint().await?;

        // Figure out which wallets are signing
        'outer: for input in pset.inputs() {
            let is_p2wpkh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_v0_p2wpkh())
                .unwrap_or(false);
            let is_p2sh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_p2sh())
                .unwrap_or(false);
            let is_p2shwpkh = is_p2sh
                && input
                    .redeem_script
                    .as_ref()
                    .map(|x| x.is_v0_p2wpkh())
                    .unwrap_or(false);
            // Singlesig
            if is_p2wpkh || is_p2shwpkh {
                // We expect exactly one element
                if let Some((fp, path)) = input.bip32_derivation.values().next() {
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
                        let name = "".to_string();
                        let version = Version::V2;
                        // TODO: cache xpubs
                        let xpub = self
                            .client
                            .get_extended_pubkey(&path, false)
                            .await
                            .expect("FIXME");
                        let key = WalletPubKey::from(((*fp, path.clone()), xpub));
                        let keys = vec![key];
                        let desc = if is_p2wpkh {
                            "wpkh(@0/**)"
                        } else {
                            "sh(wpkh(@0/**))"
                        };
                        let wallet_policy =
                            WalletPolicy::new(name, version, desc.to_string(), keys);
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            } else {
                let is_p2wsh = input
                    .witness_utxo
                    .as_ref()
                    .map(|u| u.script_pubkey.is_v0_p2wsh())
                    .unwrap_or(false);
                let details = input.witness_script.as_ref().and_then(parse_multisig);
                // Multisig
                if is_p2wsh {
                    if let Some((threshold, pubkeys)) = details {
                        let mut keys: Vec<WalletPubKey> = vec![];
                        for pubkey in pubkeys {
                            if let Some((fp, path)) = input.bip32_derivation.get(&pubkey) {
                                let mut v: Vec<ChildNumber> = path.clone().into();
                                v.truncate(3);
                                let path: DerivationPath = v.into();
                                let keysource = (*fp, path);
                                if let Some(xpub) = pset.global.xpub.iter().find_map(|(x, ks)| {
                                    if ks == &keysource {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                }) {
                                    let mut key = WalletPubKey::from((keysource, *xpub));
                                    key.multipath = Some("/**".to_string());
                                    keys.push(key);
                                } else {
                                    // Global xpub not available, cannot reconstruct the script
                                    continue 'outer;
                                }
                            } else {
                                // No keysource for pubkey in script
                                // Either the script is not ours or data is missing
                                continue 'outer;
                            }
                        }
                        let sorted = false;
                        let wallet_policy = WalletPolicy::new_multisig(
                            "todo".to_string(),
                            Version::V1,
                            AddressType::NativeSegwit,
                            threshold as usize,
                            keys,
                            sorted,
                            None,
                        )
                        .expect("FIXME");
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            }
        }

        // For each wallet, sign
        for wallet_policy in wallets.values() {
            let hmac = if wallet_policy.threshold.is_some() {
                // Register multisig wallets
                let (_id, hmac) = self
                    .client
                    .register_wallet(wallet_policy)
                    .await
                    .expect("FIXME");
                Some(hmac)
            } else {
                None
            };
            let partial_sigs = self
                .client
                .sign_psbt(pset, wallet_policy, hmac.as_ref())
                .await
                .expect("FIXME");
            n_sigs += partial_sigs.len();

            // Add sigs to pset
            for (input_idx, sig) in partial_sigs {
                let input = &mut pset.inputs_mut()[input_idx];
                for (public_key, (fp, _origin)) in &input.bip32_derivation {
                    if fp == &master_fp {
                        // TODO: user the pubkey from PartialSignature to insert in partial_sigs
                        let sig_vec = match sig {
                            PartialSignature::Sig(_, sig) => sig.to_vec(),
                            _ => panic!("FIXME: support taproot sig or raise error"),
                        };
                        input.partial_sigs.insert(*public_key, sig_vec);
                        // FIXME: handle cases where we have multiple pubkeys with master fingerprint
                        break;
                    }
                }
            }
        }

        Ok(n_sigs as u32)
    }

    pub async fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Error> {
        let r = self
            .client
            .get_extended_pubkey(path, false)
            .await
            .expect("FIXME");
        Ok(r)
    }

    pub async fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Error> {
        let r = self.client.get_master_blinding_key().await.expect("FIXME");
        Ok(r)
    }

    pub async fn fingerprint(&self) -> std::result::Result<Fingerprint, Error> {
        let r = self.client.get_master_fingerprint().await.expect("FIXME");
        Ok(r)
    }
}
