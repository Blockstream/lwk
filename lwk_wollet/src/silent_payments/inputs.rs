use elements::hashes::{hash160, Hash};
use elements::{OutPoint, Script, Transaction};

use crate::secp256k1::{Parity, PublicKey, Scalar, XOnlyPublicKey};
use crate::EC;

use super::hashes::InputsHash;
use super::SilentPaymentError;

/// The BIP341 NUMS point, used as taproot internal key when there is no key path spend.
///
/// Inputs revealing it in the control block are not considered for the shared secret, because
/// the sender does not know the corresponding secret key.
const NUMS_H: [u8; 32] = [
    0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e,
    0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
];

/// Size of a serialized outpoint: 32 bytes of txid plus 4 bytes of vout
const OUTPOINT_LEN: usize = 36;

/// A transaction input as needed to compute the silent payment shared secret.
///
/// The spent output script is needed because for taproot inputs the public key is not in the
/// witness, and because for the other input types it tells how the key must be extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilentPaymentInput {
    outpoint: OutPoint,
    prevout_script_pubkey: Script,
    script_sig: Script,
    witness: Vec<Vec<u8>>,
    known_public_key: Option<PublicKey>,
    eligible: bool,
}

impl SilentPaymentInput {
    /// Create an input from its spending data and the script of the output it spends, as a
    /// receiver scanning a transaction sees it
    pub fn new(
        outpoint: OutPoint,
        prevout_script_pubkey: Script,
        script_sig: Script,
        witness: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            outpoint,
            prevout_script_pubkey,
            script_sig,
            witness,
            known_public_key: None,
            eligible: true,
        }
    }

    /// Create an input of a transaction that has not been signed yet, as a sender building it
    /// has it: the public key is known in advance instead of being extracted from the witness.
    ///
    /// Fails if `public_key` does not match `prevout_script_pubkey`, which would make the
    /// receiver compute a different shared secret and miss the payment.
    pub fn spending(
        outpoint: OutPoint,
        prevout_script_pubkey: Script,
        public_key: PublicKey,
    ) -> Result<Self, SilentPaymentError> {
        let public_key = matching_public_key(&prevout_script_pubkey, public_key)
            .ok_or(SilentPaymentError::IneligibleInput(outpoint))?;
        Ok(Self {
            outpoint,
            prevout_script_pubkey,
            script_sig: Script::new(),
            witness: vec![],
            known_public_key: Some(public_key),
            eligible: true,
        })
    }

    /// Create an input that does not contribute its public key to the shared secret, only its
    /// outpoint: an input spending an output that is not one of the eligible types, or a
    /// peg-in, whose spending data lives on the Bitcoin chain and whose script is not known
    /// here, in which case `prevout_script_pubkey` is empty.
    ///
    /// Marking an eligible input as not eligible makes the payment undetectable by the
    /// receiver, which computes the shared secret from the signed transaction.
    pub fn other(outpoint: OutPoint, prevout_script_pubkey: Script) -> Self {
        Self {
            outpoint,
            prevout_script_pubkey,
            script_sig: Script::new(),
            witness: vec![],
            known_public_key: None,
            eligible: false,
        }
    }

    /// The outpoint spent by this input
    pub fn outpoint(&self) -> OutPoint {
        self.outpoint
    }

    /// The outpoint as it is serialized in the transaction, which is what BIP352 hashes
    pub(super) fn serialized_outpoint(&self) -> [u8; OUTPOINT_LEN] {
        let mut bytes = [0u8; OUTPOINT_LEN];
        bytes[..32].copy_from_slice(&self.outpoint.txid.to_byte_array());
        bytes[32..].copy_from_slice(&self.outpoint.vout.to_le_bytes());
        bytes
    }

    /// The public key this input contributes to the sum, if the input is eligible.
    ///
    /// Eligible inputs are P2TR (unless spent via a script path revealing the NUMS point),
    /// P2WPKH, P2SH-P2WPKH and P2PKH, always with a compressed or x-only public key.
    pub fn public_key(&self) -> Option<PublicKey> {
        if !self.eligible {
            return None;
        }
        if self.known_public_key.is_some() {
            return self.known_public_key;
        }
        let spk = &self.prevout_script_pubkey;
        if spk.is_v1_p2tr() {
            self.taproot_public_key()
        } else if spk.is_v0_p2wpkh() {
            self.witness_public_key()
        } else if spk.is_p2sh() {
            // the only eligible P2SH input is a P2SH wrapped P2WPKH, whose script sig is a
            // single push of the redeem script
            let script_sig = self.script_sig.as_bytes();
            let is_p2sh_p2wpkh = script_sig.len() == 23
                && script_sig[0] == 0x16
                && Script::from(script_sig[1..].to_vec()).is_v0_p2wpkh();
            is_p2sh_p2wpkh.then(|| self.witness_public_key()).flatten()
        } else if spk.is_p2pkh() {
            self.p2pkh_public_key()
        } else {
            None
        }
    }

    /// Whether this input spends an output this version of the protocol does not know how to
    /// handle, which makes the whole transaction ineligible
    pub(super) fn spends_unknown_witness_version(&self) -> bool {
        witness_version(&self.prevout_script_pubkey).is_some_and(|version| version > 1)
    }

    fn taproot_public_key(&self) -> Option<PublicKey> {
        let mut witness = self.witness.as_slice();
        if witness.len() > 1 && witness.last().is_some_and(|e| e.first() == Some(&0x50)) {
            // the annex is not part of the spending data
            witness = &witness[..witness.len() - 1];
        }
        if witness.len() > 1 {
            let control_block = witness.last()?;
            let internal_key = control_block.get(1..33)?;
            if internal_key == NUMS_H {
                return None;
            }
        }
        let x_only =
            XOnlyPublicKey::from_slice(&self.prevout_script_pubkey.as_bytes()[2..]).ok()?;
        Some(PublicKey::from_x_only_public_key(x_only, Parity::Even))
    }

    fn witness_public_key(&self) -> Option<PublicKey> {
        compressed_public_key(self.witness.last()?)
    }

    fn p2pkh_public_key(&self) -> Option<PublicKey> {
        // the script sig can be malleated, so instead of taking the last push we look for the
        // push matching the public key hash committed in the spent script
        let key_hash = &self.prevout_script_pubkey.as_bytes()[3..23];
        let script_sig = self.script_sig.as_bytes();
        (33..=script_sig.len())
            .rev()
            .map(|end| &script_sig[end - 33..end])
            .find(|candidate| hash160::Hash::hash(candidate).as_byte_array() == key_hash)
            .and_then(compressed_public_key)
    }
}

fn compressed_public_key(bytes: &[u8]) -> Option<PublicKey> {
    // uncompressed keys are not eligible, `PublicKey::from_slice` would accept them
    (bytes.len() == 33)
        .then(|| PublicKey::from_slice(bytes).ok())
        .flatten()
}

/// The version of a witness program, `None` if the script is not one
fn witness_version(script_pubkey: &Script) -> Option<u8> {
    let script = script_pubkey.as_bytes();
    if !(4..=42).contains(&script.len()) || script[1] as usize != script.len() - 2 {
        return None;
    }
    match script[0] {
        0 => Some(0),
        version @ 0x51..=0x60 => Some(version - 0x50),
        _ => None,
    }
}

/// The key `public_key` contributes when spending `script_pubkey`, `None` if the script is not
/// one of the eligible types or if the key is not the one committed in the script
fn matching_public_key(script_pubkey: &Script, public_key: PublicKey) -> Option<PublicKey> {
    let script = script_pubkey.as_bytes();
    let key_hash = |key: &PublicKey| hash160::Hash::hash(&key.serialize()).to_byte_array();
    if script_pubkey.is_v1_p2tr() {
        let (x_only, _) = public_key.x_only_public_key();
        // whatever the parity of the key held by the sender, the input contributes the key
        // with even parity
        (x_only.serialize() == script[2..])
            .then(|| PublicKey::from_x_only_public_key(x_only, Parity::Even))
    } else if script_pubkey.is_v0_p2wpkh() {
        (key_hash(&public_key) == script[2..22]).then_some(public_key)
    } else if script_pubkey.is_p2pkh() {
        (key_hash(&public_key) == script[3..23]).then_some(public_key)
    } else if script_pubkey.is_p2sh() {
        // the only eligible P2SH input is a P2SH wrapped P2WPKH
        let mut redeem_script = vec![0x00, 0x14];
        redeem_script.extend_from_slice(&key_hash(&public_key));
        let redeem_hash = hash160::Hash::hash(&redeem_script).to_byte_array();
        (redeem_hash == script[2..22]).then_some(public_key)
    } else {
        None
    }
}

/// The inputs of a transaction, coupled with the scripts of the outputs they spend.
///
/// The scripts must be in the same order as the transaction inputs.
pub fn transaction_inputs(
    tx: &Transaction,
    prevout_script_pubkeys: &[Script],
) -> Result<Vec<SilentPaymentInput>, SilentPaymentError> {
    if tx.input.len() != prevout_script_pubkeys.len() {
        return Err(SilentPaymentError::PrevoutsMismatch {
            inputs: tx.input.len(),
            prevouts: prevout_script_pubkeys.len(),
        });
    }
    Ok(tx
        .input
        .iter()
        .zip(prevout_script_pubkeys)
        .map(|(input, script_pubkey)| {
            if input.is_pegin {
                SilentPaymentInput::other(input.previous_output, Script::new())
            } else {
                SilentPaymentInput::new(
                    input.previous_output,
                    script_pubkey.clone(),
                    input.script_sig.clone(),
                    input.witness.script_witness.clone(),
                )
            }
        })
        .collect())
}

/// The tweak data of a transaction: `input_hash * A`, where `A` is the sum of the public keys
/// of the eligible inputs.
///
/// This is the only per transaction data a client needs to detect payments, index servers serve
/// it so that clients do not have to download and parse every transaction.
///
/// Returns `None` if the transaction cannot contain silent payment outputs, either because none
/// of its inputs is eligible or because the keys sum up to the point at infinity.
///
/// `inputs` must contain **all** the inputs of the transaction, ineligible ones included,
/// because the smallest outpoint is computed over all of them.
pub fn tweak_data(inputs: &[SilentPaymentInput]) -> Result<Option<PublicKey>, SilentPaymentError> {
    if inputs.iter().any(|i| i.spends_unknown_witness_version()) {
        // a future version of the protocol may define how to derive a key from such an input,
        // skipping the transaction now avoids having to scan it twice then
        return Ok(None);
    }
    let public_keys: Vec<_> = inputs.iter().filter_map(|i| i.public_key()).collect();
    if public_keys.is_empty() {
        return Ok(None);
    }
    let refs: Vec<_> = public_keys.iter().collect();
    let sum = match PublicKey::combine_keys(&refs) {
        Ok(sum) => sum,
        // the keys sum up to the point at infinity, no shared secret can be derived
        Err(_) => return Ok(None),
    };

    let smallest_outpoint = inputs
        .iter()
        .map(|i| i.serialized_outpoint())
        .min()
        .ok_or(SilentPaymentError::NoInputs)?;

    let input_hash = InputsHash::compute(&smallest_outpoint, &sum.serialize());
    let input_hash = Scalar::from_be_bytes(input_hash.to_byte_array())
        .map_err(|_| SilentPaymentError::InvalidInputHash)?;
    Ok(Some(sum.mul_tweak(&EC, &input_hash)?))
}

/// The tweak data of a transaction, see [`tweak_data`]
pub fn tweak_data_from_tx(
    tx: &Transaction,
    prevout_script_pubkeys: &[Script],
) -> Result<Option<PublicKey>, SilentPaymentError> {
    tweak_data(&transaction_inputs(tx, prevout_script_pubkeys)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silent_payments::test_vectors::{test_vectors, Vin};

    fn inputs(vin: &[Vin]) -> Vec<SilentPaymentInput> {
        vin.iter().map(Vin::input).collect()
    }

    #[test]
    fn bip352_tweak_data() {
        let mut checked = 0;
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let inputs = inputs(&receiving.given.vin);
                let tweak = tweak_data(&inputs).unwrap();
                match &receiving.expected.tweak {
                    Some(expected) => {
                        assert_eq!(
                            tweak.expect("tweak is expected").to_string(),
                            *expected,
                            "{}",
                            vector.comment
                        );
                    }
                    None => assert!(tweak.is_none(), "{}", vector.comment),
                }
                checked += 1;
            }
        }
        assert!(checked > 0);
    }

    /// A transaction spending an output of an unknown witness version is not scanned at all,
    /// even if it has eligible inputs
    #[test]
    fn unknown_witness_version() {
        let vector = test_vectors()
            .into_iter()
            .find(|v| v.comment == "Simple send: two inputs")
            .unwrap();
        let mut inputs = inputs(&vector.receiving[0].given.vin);
        assert!(tweak_data(&inputs).unwrap().is_some());

        let unknown = crate::silent_payments::test_vectors::script(
            "5220000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        );
        let outpoint = inputs[0].outpoint();
        inputs.push(SilentPaymentInput::other(outpoint, unknown));
        assert!(tweak_data(&inputs).unwrap().is_none());
    }

    #[test]
    fn bip352_input_public_key_sum() {
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let keys: Vec<_> = inputs(&receiving.given.vin)
                    .iter()
                    .filter_map(|i| i.public_key())
                    .collect();
                let refs: Vec<_> = keys.iter().collect();
                let sum = PublicKey::combine_keys(&refs).map(|k| k.to_string()).ok();
                assert_eq!(
                    sum, receiving.expected.input_pub_key_sum,
                    "{}",
                    vector.comment
                );
            }
        }
    }
}
