use std::collections::BTreeMap;

use elements::hashes::Hash;
use elements::{Address, AddressParams, Script};

use crate::secp256k1::{PublicKey, Scalar, SecretKey};
use crate::EC;

use super::hashes::InputsHash;
use super::scan::{blinding_key, output_tweak, taproot_script_pubkey};
use super::{SilentPaymentAddress, SilentPaymentError, SilentPaymentInput, K_MAX};

/// An output paying to a silent payment address.
///
/// On Liquid the receiver cannot be found by scanning if the output is not blinded to the key
/// derived from the shared secret, so an output is fully described by its script and its
/// blinding key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilentPaymentOutput {
    script_pubkey: Script,
    blinding_public_key: PublicKey,
}

impl SilentPaymentOutput {
    /// The taproot script this output must pay to
    pub fn script_pubkey(&self) -> &Script {
        &self.script_pubkey
    }

    /// The key this output must be blinded to
    pub fn blinding_public_key(&self) -> PublicKey {
        self.blinding_public_key
    }

    /// The confidential address this output must pay to
    pub fn address(&self, params: &'static AddressParams) -> Address {
        Address::from_script(&self.script_pubkey, Some(self.blinding_public_key), params)
            .expect("a taproot script always maps to an address")
    }
}

/// Derive the outputs paying to the given silent payment addresses.
///
/// `inputs` must contain **all** the inputs of the transaction, each one with the secret key
/// unlocking it, or `None` if the input is not eligible for silent payments (in which case it
/// only contributes its outpoint).
///
/// The returned outputs are in the same order as `recipients`. Repeating an address is allowed
/// and produces different outputs.
pub fn derive_outputs(
    inputs: &[(SilentPaymentInput, Option<SecretKey>)],
    recipients: &[SilentPaymentAddress],
) -> Result<Vec<SilentPaymentOutput>, SilentPaymentError> {
    if recipients.is_empty() {
        return Err(SilentPaymentError::NoRecipients);
    }
    let network = recipients[0].network();
    if recipients.iter().any(|r| r.network() != network) {
        return Err(SilentPaymentError::MixedNetworks);
    }

    if let Some((input, _)) = inputs
        .iter()
        .find(|(input, _)| input.spends_unknown_witness_version())
    {
        // the receiver would not scan the transaction at all
        return Err(SilentPaymentError::IneligibleInput(input.outpoint()));
    }
    let secret_key = inputs_secret_key(inputs)?;
    let smallest_outpoint = inputs
        .iter()
        .map(|(input, _)| input.serialized_outpoint())
        .min()
        .ok_or(SilentPaymentError::NoInputs)?;
    let public_key = PublicKey::from_secret_key(&EC, &secret_key);
    let input_hash = InputsHash::compute(&smallest_outpoint, &public_key.serialize());
    let input_hash = Scalar::from_be_bytes(input_hash.to_byte_array())
        .map_err(|_| SilentPaymentError::InvalidInputHash)?;
    let secret_key = secret_key.mul_tweak(&input_hash)?;

    // outputs paid to the same scan key share the shared secret and are told apart by the
    // counter `k`
    let mut groups: BTreeMap<[u8; 33], Vec<(usize, PublicKey)>> = BTreeMap::new();
    for (i, recipient) in recipients.iter().enumerate() {
        groups
            .entry(recipient.scan_public_key().serialize())
            .or_default()
            .push((i, recipient.spend_public_key()));
    }

    let mut outputs = vec![None; recipients.len()];
    for (scan_public_key, group) in groups {
        if group.len() > K_MAX as usize {
            return Err(SilentPaymentError::TooManyOutputsPerScanKey);
        }
        let scan_public_key = PublicKey::from_slice(&scan_public_key)?;
        let shared_secret = scan_public_key.mul_tweak(&EC, &Scalar::from(secret_key))?;
        for (k, (i, spend_public_key)) in group.into_iter().enumerate() {
            let k = k as u32;
            let tweak = output_tweak(&shared_secret, k)?;
            let output_key = spend_public_key.add_exp_tweak(&EC, &Scalar::from(tweak))?;
            outputs[i] = Some(SilentPaymentOutput {
                script_pubkey: taproot_script_pubkey(&output_key),
                blinding_public_key: PublicKey::from_secret_key(
                    &EC,
                    &blinding_key(&shared_secret, k)?,
                ),
            });
        }
    }

    Ok(outputs.into_iter().flatten().collect())
}

/// The sum of the secret keys of the eligible inputs, `a`
fn inputs_secret_key(
    inputs: &[(SilentPaymentInput, Option<SecretKey>)],
) -> Result<SecretKey, SilentPaymentError> {
    // `None` is zero, which is not a valid secret key but is a valid intermediate sum
    let mut sum: Option<SecretKey> = None;
    let mut eligible = false;
    for (input, secret_key) in inputs {
        let Some(public_key) = input.public_key() else {
            continue;
        };
        eligible = true;
        let secret_key = secret_key
            .ok_or_else(|| SilentPaymentError::MissingInputSecretKey(input.outpoint()))?;
        // taproot inputs contribute the key with even parity, so the secret key of an odd key
        // must be negated
        let secret_key = if PublicKey::from_secret_key(&EC, &secret_key) == public_key {
            secret_key
        } else {
            let negated = secret_key.negate();
            if PublicKey::from_secret_key(&EC, &negated) != public_key {
                return Err(SilentPaymentError::WrongInputSecretKey(input.outpoint()));
            }
            negated
        };
        sum = match sum {
            None => Some(secret_key),
            Some(sum) => sum.add_tweak(&Scalar::from(secret_key)).ok(),
        };
    }
    if !eligible {
        return Err(SilentPaymentError::NoEligibleInputs);
    }
    sum.ok_or(SilentPaymentError::InputsSumToInfinity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silent_payments::test_vectors::{test_vectors, Vin};
    use elements::hex::ToHex;

    #[test]
    fn bip352_sending() {
        let mut checked = 0;
        for vector in test_vectors() {
            for sending in &vector.sending {
                let inputs: Vec<_> = sending
                    .given
                    .vin
                    .iter()
                    .map(|vin| (vin.input(), vin.secret_key()))
                    .collect();
                let recipients: Vec<_> = sending
                    .given
                    .recipients
                    .iter()
                    .flat_map(|r| vec![r.address(); r.count])
                    .collect();

                let alternatives = &sending.expected.outputs;
                let outputs = derive_outputs(&inputs, &recipients);
                if alternatives.iter().all(|a| a.is_empty()) {
                    assert!(outputs.is_err(), "{}", vector.comment);
                    checked += 1;
                    continue;
                }
                let mut outputs: Vec<String> = outputs
                    .unwrap_or_else(|e| panic!("{}: {e}", vector.comment))
                    .iter()
                    .map(|o| o.script_pubkey().as_bytes()[2..].to_hex())
                    .collect();
                outputs.sort();
                let matches = alternatives.iter().any(|alternative| {
                    let mut alternative = alternative.clone();
                    alternative.sort();
                    alternative == outputs
                });
                assert!(matches, "{}: {outputs:?}", vector.comment);
                checked += 1;
            }
        }
        assert!(checked > 0);
    }

    /// Sender and receiver must derive the same blinding key, or the receiver cannot unblind
    /// what it receives
    #[test]
    fn blinding_key_roundtrip() {
        let mut checked = 0;
        for vector in test_vectors() {
            for (sending, receiving) in vector.sending.iter().zip(&vector.receiving) {
                let inputs: Vec<_> = sending
                    .given
                    .vin
                    .iter()
                    .map(|vin| (vin.input(), vin.secret_key()))
                    .collect();
                let recipients: Vec<_> = sending
                    .given
                    .recipients
                    .iter()
                    .flat_map(|r| vec![r.address(); r.count])
                    .collect();
                let Ok(outputs) = derive_outputs(&inputs, &recipients) else {
                    continue;
                };

                let scanner = receiving.scanner();
                let scan_inputs: Vec<_> = receiving.given.vin.iter().map(Vin::input).collect();
                let Some(tweak_data) = super::super::tweak_data(&scan_inputs).unwrap() else {
                    continue;
                };
                let tx = crate::silent_payments::test_vectors::transaction(
                    &scan_inputs,
                    &outputs
                        .iter()
                        .map(|o| o.script_pubkey().clone())
                        .collect::<Vec<_>>(),
                );
                let found = scanner
                    .scan_transaction_with_tweak_data(&tx, &tweak_data)
                    .unwrap();
                for output in &found {
                    let expected = outputs
                        .iter()
                        .find(|o| o.script_pubkey() == output.script_pubkey())
                        .expect("the scanner found an output we did not create");
                    assert_eq!(
                        PublicKey::from_secret_key(&EC, &output.blinding_key()),
                        expected.blinding_public_key(),
                        "{}",
                        vector.comment
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }
}
