use std::collections::HashMap;

use elements::confidential::{Asset, AssetBlindingFactor, Nonce, Value, ValueBlindingFactor};
use elements::hashes::Hash;
use elements::{OutPoint, Script, Transaction, TxOut, TxOutSecrets};
use serde::{Deserialize, Serialize};

use crate::secp256k1::{Parity, PublicKey, Scalar, SecretKey, XOnlyPublicKey};
use crate::{ExternalUtxo, EC};

use super::hashes::{BlindingHash, LabelHash, SharedSecretHash};
use super::inputs::tweak_data_from_tx;
use super::{SilentPaymentAddress, SilentPaymentError, SilentPaymentNetwork};

/// Maximum number of outputs a transaction can pay to the same scan key, as defined by BIP352
pub const K_MAX: u32 = 2323;

/// The label reserved for the change of a silent payment wallet
pub const CHANGE_LABEL: u32 = 0;

/// The weight needed to spend a silent payment output: a taproot key path signature
const SPENDING_WEIGHT: usize = 66;

/// Detects the outputs of a transaction paying to a silent payment address.
///
/// It needs the scan secret key and the spend public key, in other words what a watch only
/// wallet is expected to hold: it can tell which outputs are received but it cannot spend them.
#[derive(Debug, Clone)]
pub struct SilentPaymentScanner {
    scan_secret_key: SecretKey,
    spend_public_key: PublicKey,
    labels: HashMap<[u8; 33], Label>,
}

#[derive(Debug, Clone, Copy)]
struct Label {
    m: u32,
    tweak: SecretKey,
}

/// An output of a transaction paying to a silent payment address of the wallet
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SilentPaymentTxOut {
    outpoint: OutPoint,
    txout: TxOut,
    tweak: SecretKey,
    label: Option<u32>,
    blinding_key: SecretKey,
    unblinded: Option<TxOutSecrets>,
}

impl SilentPaymentTxOut {
    /// The outpoint of this output
    pub fn outpoint(&self) -> OutPoint {
        self.outpoint
    }

    /// The transaction output
    pub fn txout(&self) -> &TxOut {
        &self.txout
    }

    /// The script pubkey of this output, a taproot script
    pub fn script_pubkey(&self) -> &Script {
        &self.txout.script_pubkey
    }

    /// The value to add to the spend secret key to obtain the secret key of this output
    pub fn tweak(&self) -> SecretKey {
        self.tweak
    }

    /// The label of the address this output was paid to, [`CHANGE_LABEL`] is the change
    pub fn label(&self) -> Option<u32> {
        self.label
    }

    /// The secret key needed to unblind this output, it is derived from the shared secret so
    /// that the sender can blind an output that only the receiver knows how to find
    pub fn blinding_key(&self) -> SecretKey {
        self.blinding_key
    }

    /// The unblinded asset and value of this output, `None` if unblinding failed
    pub fn unblinded(&self) -> Option<&TxOutSecrets> {
        self.unblinded.as_ref()
    }

    /// The secret key spending this output, given the spend secret key of the wallet
    pub fn spending_secret_key(
        &self,
        spend_secret_key: &SecretKey,
    ) -> Result<SecretKey, SilentPaymentError> {
        Ok(spend_secret_key.add_tweak(&Scalar::from(self.tweak))?)
    }

    /// This output as an input for [`crate::TxBuilder::add_external_utxos`], `None` if it
    /// could not be unblinded
    pub fn to_external_utxo(&self) -> Option<ExternalUtxo> {
        Some(ExternalUtxo {
            outpoint: self.outpoint,
            txout: self.txout.clone(),
            tx: None,
            unblinded: *self.unblinded.as_ref()?,
            max_weight_to_satisfy: SPENDING_WEIGHT,
        })
    }
}

impl SilentPaymentScanner {
    /// Create a scanner for the given keys
    pub fn new(scan_secret_key: SecretKey, spend_public_key: PublicKey) -> Self {
        Self {
            scan_secret_key,
            spend_public_key,
            labels: HashMap::new(),
        }
    }

    /// Also detect the outputs paid to the address labelled with `m`, see
    /// [`SilentPaymentScanner::labelled_address`]
    pub fn add_label(&mut self, m: u32) -> Result<(), SilentPaymentError> {
        let tweak = label_tweak(&self.scan_secret_key, m)?;
        let point = PublicKey::from_secret_key(&EC, &tweak);
        self.labels.insert(point.serialize(), Label { m, tweak });
        Ok(())
    }

    /// The scan public key
    pub fn scan_public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&EC, &self.scan_secret_key)
    }

    /// The spend public key
    pub fn spend_public_key(&self) -> PublicKey {
        self.spend_public_key
    }

    /// The address to give out to receive payments
    pub fn address(&self, network: SilentPaymentNetwork) -> SilentPaymentAddress {
        SilentPaymentAddress::new(self.scan_public_key(), self.spend_public_key, network)
    }

    /// The address labelled with `m`, payments to it are detected only if the label has been
    /// added with [`SilentPaymentScanner::add_label`].
    ///
    /// Labels allow to tell apart payments made to different addresses of the same wallet
    /// without publishing more than one set of keys, `m` = [`CHANGE_LABEL`] is reserved for the
    /// change and must not be given out.
    pub fn labelled_address(
        &self,
        network: SilentPaymentNetwork,
        m: u32,
    ) -> Result<SilentPaymentAddress, SilentPaymentError> {
        let tweak = label_tweak(&self.scan_secret_key, m)?;
        let spend_public_key = self
            .spend_public_key
            .add_exp_tweak(&EC, &Scalar::from(tweak))?;
        Ok(SilentPaymentAddress::new(
            self.scan_public_key(),
            spend_public_key,
            network,
        ))
    }

    /// The shared secret between the sender and the receiver, `input_hash * b_scan * A`
    fn shared_secret(&self, tweak_data: &PublicKey) -> Result<PublicKey, SilentPaymentError> {
        Ok(tweak_data.mul_tweak(&EC, &Scalar::from(self.scan_secret_key))?)
    }

    /// The scripts a transaction with the given tweak data would pay to, if it paid to this
    /// wallet: the first one for the address itself and one for each label.
    ///
    /// A light client matches these against a block filter to know whether it is worth
    /// downloading a transaction and scanning it with
    /// [`SilentPaymentScanner::scan_transaction_with_tweak_data`].
    pub fn candidate_script_pubkeys(
        &self,
        tweak_data: &PublicKey,
    ) -> Result<Vec<Script>, SilentPaymentError> {
        let shared_secret = self.shared_secret(tweak_data)?;
        let (_, output_key) = self.output_key(&shared_secret, 0)?;
        let mut scripts = vec![taproot_script_pubkey(&output_key)];
        for label in self.labels.values() {
            let labelled = output_key.add_exp_tweak(&EC, &Scalar::from(label.tweak))?;
            scripts.push(taproot_script_pubkey(&labelled));
        }
        Ok(scripts)
    }

    /// Detect the outputs of `tx` paying to this wallet.
    ///
    /// `prevout_script_pubkeys` are the scripts of the outputs spent by `tx`, in the same order
    /// as its inputs. They are needed because taproot inputs do not reveal their public key in
    /// the witness.
    pub fn scan_transaction(
        &self,
        tx: &Transaction,
        prevout_script_pubkeys: &[Script],
    ) -> Result<Vec<SilentPaymentTxOut>, SilentPaymentError> {
        match tweak_data_from_tx(tx, prevout_script_pubkeys)? {
            Some(tweak_data) => self.scan_transaction_with_tweak_data(tx, &tweak_data),
            None => Ok(vec![]),
        }
    }

    /// Detect the outputs of `tx` paying to this wallet, using the tweak data obtained from an
    /// index server instead of the outputs spent by `tx`, see
    /// [`crate::silent_payments::tweak_data`]
    pub fn scan_transaction_with_tweak_data(
        &self,
        tx: &Transaction,
        tweak_data: &PublicKey,
    ) -> Result<Vec<SilentPaymentTxOut>, SilentPaymentError> {
        let shared_secret = self.shared_secret(tweak_data)?;
        let txid = tx.txid();

        let mut candidates: Vec<(u32, XOnlyPublicKey)> = tx
            .output
            .iter()
            .enumerate()
            .filter_map(|(vout, txout)| {
                let script_pubkey = &txout.script_pubkey;
                if !script_pubkey.is_v1_p2tr() {
                    return None;
                }
                let key = XOnlyPublicKey::from_slice(&script_pubkey.as_bytes()[2..]).ok()?;
                Some((vout as u32, key))
            })
            .collect();

        let mut found = vec![];
        for k in 0..K_MAX {
            if candidates.is_empty() {
                break;
            }
            let (tweak, output_key) = self.output_key(&shared_secret, k)?;
            let Some((position, label)) = self.match_output(&candidates, &output_key)? else {
                break;
            };
            let (vout, _) = candidates.remove(position);
            let tweak = match label {
                Some(label) => tweak.add_tweak(&Scalar::from(label.tweak))?,
                None => tweak,
            };
            let blinding_key = blinding_key(&shared_secret, k)?;
            let txout = tx.output[vout as usize].clone();
            found.push(SilentPaymentTxOut {
                outpoint: OutPoint::new(txid, vout),
                unblinded: unblind(&txout, &blinding_key),
                txout,
                tweak,
                label: label.map(|l| l.m),
                blinding_key,
            });
        }
        // outputs are found in the order they were created by the sender, return them in the
        // order they appear in the transaction
        found.sort_by_key(|txout| txout.outpoint.vout);
        Ok(found)
    }

    /// `t_k` and `P_k = B_spend + t_k * G`
    fn output_key(
        &self,
        shared_secret: &PublicKey,
        k: u32,
    ) -> Result<(SecretKey, PublicKey), SilentPaymentError> {
        let tweak = output_tweak(shared_secret, k)?;
        let output_key = self
            .spend_public_key
            .add_exp_tweak(&EC, &Scalar::from(tweak))?;
        Ok((tweak, output_key))
    }

    /// The position of the output paying to `output_key`, and the label it was paid to
    fn match_output(
        &self,
        candidates: &[(u32, XOnlyPublicKey)],
        output_key: &PublicKey,
    ) -> Result<Option<(usize, Option<Label>)>, SilentPaymentError> {
        let (expected, _) = output_key.x_only_public_key();
        if let Some(position) = candidates.iter().position(|(_, key)| *key == expected) {
            return Ok(Some((position, None)));
        }
        if self.labels.is_empty() {
            return Ok(None);
        }
        let negated_output_key = output_key.negate(&EC);
        for (position, (_, key)) in candidates.iter().enumerate() {
            // the label is the difference between the output and the key we derived, both
            // parities of the output must be tried since the transaction only commits to the
            // x coordinate
            let key = PublicKey::from_x_only_public_key(*key, Parity::Even);
            for candidate in [key, key.negate(&EC)] {
                let Ok(label) = candidate.combine(&negated_output_key) else {
                    continue;
                };
                if let Some(label) = self.labels.get(&label.serialize()) {
                    return Ok(Some((position, Some(*label))));
                }
            }
        }
        Ok(None)
    }
}

/// `t_k = hash_BIP0352/SharedSecret(ser_P(ecdh_shared_secret) || ser32(k))`
pub(super) fn output_tweak(
    shared_secret: &PublicKey,
    k: u32,
) -> Result<SecretKey, SilentPaymentError> {
    let hash = SharedSecretHash::compute(&shared_secret.serialize(), k);
    SecretKey::from_slice(hash.as_byte_array())
        .map_err(|_| SilentPaymentError::InvalidSharedSecretHash)
}

/// `label_tweak = hash_BIP0352/Label(ser256(b_scan) || ser32(m))`
pub(super) fn label_tweak(
    scan_secret_key: &SecretKey,
    m: u32,
) -> Result<SecretKey, SilentPaymentError> {
    let hash = LabelHash::compute(&scan_secret_key.secret_bytes(), m);
    SecretKey::from_slice(hash.as_byte_array()).map_err(|_| SilentPaymentError::InvalidLabelHash)
}

/// The secret key the output is blinded to, the Liquid specific part of the derivation
pub(super) fn blinding_key(
    shared_secret: &PublicKey,
    k: u32,
) -> Result<SecretKey, SilentPaymentError> {
    let hash = BlindingHash::compute(&shared_secret.serialize(), k);
    SecretKey::from_slice(hash.as_byte_array()).map_err(|_| SilentPaymentError::InvalidBlindingHash)
}

/// The taproot script paying to `output_key`, silent payment outputs commit to the key as it
/// is, without applying the BIP341 tweak
pub(super) fn taproot_script_pubkey(output_key: &PublicKey) -> Script {
    let (x_only, _) = output_key.x_only_public_key();
    let mut script = Vec::with_capacity(34);
    script.push(0x51);
    script.push(0x20);
    script.extend_from_slice(&x_only.serialize());
    Script::from(script)
}

fn unblind(txout: &TxOut, blinding_key: &SecretKey) -> Option<TxOutSecrets> {
    match (txout.asset, txout.value, txout.nonce) {
        (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
            txout.unblind(&EC, *blinding_key).ok()
        }
        (Asset::Explicit(asset), Value::Explicit(value), _) => Some(TxOutSecrets::new(
            asset,
            AssetBlindingFactor::zero(),
            value,
            ValueBlindingFactor::zero(),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silent_payments::test_vectors::{
        encode_bip352_address, taproot_script, test_vectors, transaction, Vin,
    };
    use elements::hex::ToHex;

    #[test]
    fn bip352_receiving() {
        let mut checked = 0;
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let scanner = receiving.scanner();
                let inputs: Vec<_> = receiving.given.vin.iter().map(Vin::input).collect();
                let scripts: Vec<_> = receiving
                    .given
                    .outputs
                    .iter()
                    .map(|o| taproot_script(o))
                    .collect();
                let tx = transaction(&inputs, &scripts);

                let found = match super::super::tweak_data(&inputs).unwrap() {
                    Some(tweak_data) => scanner
                        .scan_transaction_with_tweak_data(&tx, &tweak_data)
                        .unwrap(),
                    None => vec![],
                };

                assert_eq!(
                    found.len(),
                    receiving.expected_outputs(),
                    "{}",
                    vector.comment
                );
                let spend_secret_key = receiving.given.key_material.spend_priv_key.parse().unwrap();
                for (found, expected) in found.iter().zip(&receiving.expected.outputs) {
                    assert_eq!(
                        found.tweak().secret_bytes().to_hex(),
                        expected.priv_key_tweak,
                        "{}",
                        vector.comment
                    );
                    assert_eq!(
                        found.script_pubkey().as_bytes()[2..].to_hex(),
                        expected.pub_key,
                        "{}",
                        vector.comment
                    );
                    // the wallet can spend what it detects
                    let secret_key = found.spending_secret_key(&spend_secret_key).unwrap();
                    let (x_only, _) = secret_key.x_only_public_key(&EC);
                    assert_eq!(x_only.serialize().to_hex(), expected.pub_key);
                }
                checked += 1;
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn bip352_addresses() {
        let mut checked = 0;
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let scanner = receiving.scanner();
                let network = SilentPaymentNetwork::Liquid;
                let mut addresses = vec![scanner.address(network)];
                for label in &receiving.given.labels {
                    addresses.push(scanner.labelled_address(network, *label).unwrap());
                }
                let addresses: Vec<_> = addresses.iter().map(encode_bip352_address).collect();
                assert_eq!(
                    addresses, receiving.expected.addresses,
                    "{}",
                    vector.comment
                );
                checked += addresses.len();
            }
        }
        assert_eq!(checked, 44);
    }

    #[test]
    fn bip352_candidate_script_pubkeys() {
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let scanner = receiving.scanner();
                let inputs: Vec<_> = receiving.given.vin.iter().map(Vin::input).collect();
                let Some(tweak_data) = super::super::tweak_data(&inputs).unwrap() else {
                    continue;
                };
                let candidates = scanner.candidate_script_pubkeys(&tweak_data).unwrap();
                assert_eq!(candidates.len(), receiving.given.labels.len() + 1);

                // a transaction paying to this wallet always pays to one of the candidates,
                // which is what makes filter matching work
                let scripts: Vec<_> = receiving
                    .given
                    .outputs
                    .iter()
                    .map(|o| taproot_script(o))
                    .collect();
                let tx = transaction(&inputs, &scripts);
                let found = scanner
                    .scan_transaction_with_tweak_data(&tx, &tweak_data)
                    .unwrap();
                if !found.is_empty() {
                    assert!(
                        found
                            .iter()
                            .any(|txout| candidates.contains(txout.script_pubkey())),
                        "{}",
                        vector.comment
                    );
                }
            }
        }
    }
}
