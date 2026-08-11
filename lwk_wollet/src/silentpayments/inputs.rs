//! Silent-payment input aggregation.

use crate::elements::OutPoint;
use crate::hashes::{Hash, HashEngine};
use crate::secp256k1::{PublicKey, Scalar, SecretKey};
use crate::silentpayments::tags::InputsHash;
use crate::util::EC;
use std::collections::HashMap;

/// Computes BIP-352 input hashes.
pub(crate) struct InputHasher;

impl InputHasher {
    /// `input_hash = H_BIP0352/Inputs(outpoint_L || A)`.
    ///
    /// `outpoint_l` is the serialization of the lexicographically smallest input
    /// outpoint; `a_sum_pubkey` is `A = a·G` (the sum of eligible input pubkeys).
    pub(crate) fn hash(outpoint_l: &[u8], a_sum_pubkey: &PublicKey) -> Scalar {
        let mut eng = InputsHash::engine();
        eng.input(outpoint_l);
        eng.input(&a_sum_pubkey.serialize());
        let h = InputsHash::from_engine(eng);
        // BIP-352 treats the hash directly as a scalar.
        Scalar::from_be_bytes(h.to_byte_array()).expect("input hash within curve order")
    }

    /// BIP-352's 36-byte form: `txid (32, internal) || vout (4, LE)`.
    pub(crate) fn serialize_outpoint(outpoint: &crate::elements::OutPoint) -> Vec<u8> {
        crate::elements::encode::serialize(outpoint)
    }

    /// Hashes the smallest serialized outpoint.
    pub(crate) fn hash_over<'a>(
        outpoints: impl Iterator<Item = &'a crate::elements::OutPoint>,
        a_sum_pubkey: &PublicKey,
    ) -> Scalar {
        let outpoint_l = outpoints
            .map(Self::serialize_outpoint)
            .min()
            .expect("caller checked inputs are non-empty");
        Self::hash(&outpoint_l, a_sum_pubkey)
    }
}

/// Eligible input key and spend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    /// A key-path Taproot input: normalize to even-Y before summing.
    Taproot(SecretKey),
    /// A P2WPKH / P2SH-P2WPKH input: sum as-is.
    Plain(SecretKey),
}

impl InputKey {
    /// Normalized secret key.
    pub fn normalized(&self) -> SecretKey {
        match self {
            InputKey::Taproot(sk) => {
                if sk.public_key(&EC).x_only_public_key().1
                    == crate::elements::secp256k1_zkp::Parity::Odd
                {
                    sk.negate()
                } else {
                    *sk
                }
            }
            InputKey::Plain(sk) => *sk,
        }
    }

    /// The public key an observer recovers for this input, i.e. `normalized()·G`.
    pub fn public_key(&self) -> PublicKey {
        self.normalized().public_key(&EC)
    }
}

/// Errors aggregating silent-payment inputs.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentPaymentInputError {
    /// No eligible inputs were provided.
    #[error("no eligible silent payment inputs")]
    NoInputs,

    /// A selected input should contribute to the sender sum, but its private key
    /// was not supplied by the signer/provider.
    #[error("missing private key for an eligible silent payment input")]
    MissingKey,

    /// The summed private key is zero (the inputs cancel out); the payment is
    /// undefined per BIP-352 and the transaction must be aborted.
    #[error("eligible input keys sum to zero")]
    SumIsZero,

    /// More outputs than BIP-352's `K_max` allows in one recipient group; the
    /// receiver is forbidden to scan that far and could not spend them.
    #[error("too many silent payment outputs in one transaction (limit is K_max = 2323)")]
    TooManyOutputs,
}

/// Sender-side aggregated input data.
#[derive(Debug, Clone, Copy)]
pub struct SilentPaymentInputs {
    /// `a = Σ a_i` over eligible inputs.
    pub a_sum: SecretKey,
    /// `A = a·G`.
    pub a_pubkey: PublicKey,
    /// `input_hash = H_BIP0352/Inputs(outpoint_L || A)`.
    pub input_hash: Scalar,
}

impl SilentPaymentInputs {
    /// Aggregate `(outpoint, private_key)` pairs into `a`, `A`, and `input_hash`.
    pub fn aggregate(
        inputs: &[(crate::elements::OutPoint, SecretKey)],
    ) -> Result<Self, SilentPaymentInputError> {
        let tagged: Vec<_> = inputs
            .iter()
            .map(|(o, sk)| (*o, InputKey::Plain(*sk)))
            .collect();
        Self::aggregate_keys(&tagged)
    }

    /// Aggregates typed eligible inputs.
    pub fn aggregate_keys(
        inputs: &[(crate::elements::OutPoint, InputKey)],
    ) -> Result<Self, SilentPaymentInputError> {
        Self::aggregate_with_extra_outpoints(inputs, &[])
    }

    /// Aggregates eligible inputs and keyless-input outpoints.
    pub fn aggregate_with_extra_outpoints(
        inputs: &[(crate::elements::OutPoint, InputKey)],
        extra_outpoints: &[crate::elements::OutPoint],
    ) -> Result<Self, SilentPaymentInputError> {
        let (first, rest) = inputs
            .split_first()
            .ok_or(SilentPaymentInputError::NoInputs)?;

        // `SecretKey` cannot represent an intermediate zero sum.
        let mut a_sum = Some(first.1.normalized());
        for (_, key) in rest {
            let next = key.normalized();
            a_sum = match a_sum {
                Some(current) => current
                    .add_tweak(&Scalar::from_be_bytes(next.secret_bytes()).expect("scalar"))
                    .ok(),
                None => Some(next),
            };
        }
        let a_sum = a_sum.ok_or(SilentPaymentInputError::SumIsZero)?;
        let a_pubkey = a_sum.public_key(&EC);

        let input_hash = InputHasher::hash_over(
            inputs.iter().map(|(o, _)| o).chain(extra_outpoints.iter()),
            &a_pubkey,
        );

        Ok(SilentPaymentInputs {
            a_sum,
            a_pubkey,
            input_hash,
        })
    }

    /// The observer's view of these same inputs.
    pub fn observed(&self) -> ObservedInputs {
        ObservedInputs {
            a_pubkey: self.a_pubkey,
            input_hash: self.input_hash,
        }
    }
}

/// Observer-side aggregated public input data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedInputs {
    /// `A = Σ A_i`.
    pub a_pubkey: PublicKey,
    /// `input_hash = H_BIP0352/Inputs(outpoint_L || A)`.
    pub input_hash: Scalar,
}

impl ObservedInputs {
    /// Aggregate `(outpoint, pubkey)` pairs into `A` and `input_hash`.
    pub fn aggregate(
        inputs: &[(crate::elements::OutPoint, PublicKey)],
    ) -> Result<Self, SilentPaymentInputError> {
        Self::aggregate_with_extra_outpoints(inputs, &[])
    }

    /// Aggregates observed inputs and keyless-input outpoints.
    pub fn aggregate_with_extra_outpoints(
        inputs: &[(crate::elements::OutPoint, PublicKey)],
        extra_outpoints: &[crate::elements::OutPoint],
    ) -> Result<Self, SilentPaymentInputError> {
        let (first, rest) = inputs
            .split_first()
            .ok_or(SilentPaymentInputError::NoInputs)?;

        // An intermediate point at infinity is the additive identity.
        let mut a_pubkey = Some(first.1);
        for (_, pk) in rest {
            a_pubkey = match a_pubkey {
                Some(current) => current.combine(pk).ok(),
                None => Some(*pk),
            };
        }
        let a_pubkey = a_pubkey.ok_or(SilentPaymentInputError::SumIsZero)?;

        let input_hash = InputHasher::hash_over(
            inputs.iter().map(|(o, _)| o).chain(extra_outpoints.iter()),
            &a_pubkey,
        );

        Ok(ObservedInputs {
            a_pubkey,
            input_hash,
        })
    }
}

/// Result of classifying one selected transaction input for silent payments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKeyResult {
    /// The input contributes this private key to the sender-side sum.
    Eligible(InputKey),
    /// The input belongs to the transaction but contributes no key, such as a peg-in.
    Ineligible,
    /// The input is eligible in principle, but its private key was not supplied.
    Missing,
}

/// Supplies silent-payment input classification and, where available, key material.
pub trait SilentPaymentInputProvider {
    /// Classifies `outpoint` and supplies available key material.
    fn input_key(&self, outpoint: &OutPoint) -> InputKeyResult;
}

/// A [`SilentPaymentInputProvider`] backed by an in-memory map.
#[derive(Debug, Clone, Default)]
pub struct MapInputProvider {
    keys: HashMap<OutPoint, InputKey>,
}

impl MapInputProvider {
    /// An empty provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the key spending `outpoint`.
    pub fn insert(mut self, outpoint: OutPoint, key: InputKey) -> Self {
        self.keys.insert(outpoint, key);
        self
    }
}

impl FromIterator<(OutPoint, InputKey)> for MapInputProvider {
    fn from_iter<T: IntoIterator<Item = (OutPoint, InputKey)>>(iter: T) -> Self {
        Self {
            keys: iter.into_iter().collect(),
        }
    }
}

impl SilentPaymentInputProvider for MapInputProvider {
    fn input_key(&self, outpoint: &OutPoint) -> InputKeyResult {
        self.keys
            .get(outpoint)
            .copied()
            .map(InputKeyResult::Eligible)
            .unwrap_or(InputKeyResult::Missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lwk_test_util::ElementsTestData as Data;

    #[test]
    fn map_provider_reports_known_and_unknown_inputs() {
        let known = Data::outpoint(0x10, 0);
        let unknown = Data::outpoint(0x99, 7);

        let provider =
            MapInputProvider::new().insert(known, InputKey::Plain(Data::secret_key(0x31)));

        assert_eq!(
            provider.input_key(&known),
            InputKeyResult::Eligible(InputKey::Plain(Data::secret_key(0x31)))
        );
        assert_eq!(provider.input_key(&unknown), InputKeyResult::Missing);

        // Taproot keys keep their tag so the even-Y normalization is not lost.
        let provider: MapInputProvider = [(known, InputKey::Taproot(Data::secret_key(0x31)))]
            .into_iter()
            .collect();
        assert_eq!(
            provider.input_key(&known),
            InputKeyResult::Eligible(InputKey::Taproot(Data::secret_key(0x31)))
        );
    }

    #[test]
    fn input_aggregation_sender_and_observer_agree() {
        let inputs_sk = [
            (Data::outpoint(0x30, 1), Data::secret_key(0x30)),
            (Data::outpoint(0x10, 0), Data::secret_key(0x31)), // smallest txid → outpoint_L
            (Data::outpoint(0x20, 7), Data::secret_key(0x32)),
        ];
        let agg = SilentPaymentInputs::aggregate(&inputs_sk).unwrap();

        let inputs_pk: Vec<_> = inputs_sk
            .iter()
            .map(|(o, s)| (*o, s.public_key(&EC)))
            .collect();
        let obs = ObservedInputs::aggregate(&inputs_pk).unwrap();

        assert_eq!(
            agg.a_pubkey, obs.a_pubkey,
            "A mismatch between sender and observer"
        );
        assert_eq!(
            agg.input_hash, obs.input_hash,
            "input_hash mismatch between sender and observer"
        );
        assert_eq!(agg.observed(), obs);
        assert_eq!(agg.a_sum.public_key(&EC), agg.a_pubkey);

        let reversed: Vec<_> = inputs_sk.iter().rev().copied().collect();
        let shuffled = SilentPaymentInputs::aggregate(&reversed).unwrap();
        assert_eq!(shuffled.input_hash, agg.input_hash);
        assert_eq!(shuffled.a_pubkey, agg.a_pubkey);
    }

    /// `outpoint_L` uses serialized, not `OutPoint`, ordering.
    #[test]
    fn outpoint_l_uses_serialized_byte_order_not_outpoint_ord() {
        let low_vout = Data::outpoint(0x10, 1); // serializes ...01000000
        let high_vout = Data::outpoint(0x10, 256); // serializes ...00010000

        // The two orderings must genuinely disagree, or the test is vacuous.
        assert!(low_vout < high_vout, "numerically vout 1 < vout 256");
        assert!(
            InputHasher::serialize_outpoint(&high_vout)
                < InputHasher::serialize_outpoint(&low_vout),
            "as bytes, vout 256 sorts before vout 1"
        );

        let a = Data::secret_key(0x31);
        let agg = SilentPaymentInputs::aggregate(&[(low_vout, a), (high_vout, a)]).unwrap();

        let expected =
            InputHasher::hash(&InputHasher::serialize_outpoint(&high_vout), &agg.a_pubkey);
        assert_eq!(
            agg.input_hash, expected,
            "input_hash must use the byte-wise smallest outpoint"
        );

        // Not the hash the `OutPoint: Ord` minimum would produce.
        let wrong = InputHasher::hash(&InputHasher::serialize_outpoint(&low_vout), &agg.a_pubkey);
        assert_ne!(agg.input_hash, wrong);

        let obs = ObservedInputs::aggregate(&[
            (low_vout, a.public_key(&EC)),
            (high_vout, a.public_key(&EC)),
        ])
        .unwrap();
        assert_eq!(obs.input_hash, agg.input_hash);
    }

    /// ELIP `test_taproot_even_y_negation`.
    #[test]
    fn taproot_input_keys_are_negated_to_even_y() {
        use crate::elements::secp256k1_zkp::Parity;

        let odd = (1u8..=0xFF)
            .map(Data::secret_key)
            .find(|k| k.public_key(&EC).x_only_public_key().1 == Parity::Odd)
            .expect("an odd-Y key exists");
        let even = (1u8..=0xFF)
            .map(Data::secret_key)
            .find(|k| k.public_key(&EC).x_only_public_key().1 == Parity::Even)
            .expect("an even-Y key exists");

        assert_eq!(InputKey::Taproot(odd).normalized(), odd.negate());
        assert_ne!(
            InputKey::Taproot(odd).normalized(),
            InputKey::Plain(odd).normalized()
        );
        // Negation preserves the x-only key.
        assert_eq!(
            InputKey::Taproot(odd).public_key().x_only_public_key().0,
            odd.public_key(&EC).x_only_public_key().0
        );

        assert_eq!(InputKey::Taproot(even).normalized(), even);
        assert_eq!(
            InputKey::Taproot(even).normalized(),
            InputKey::Plain(even).normalized()
        );

        let op = Data::outpoint(0x10, 0);
        let agg = SilentPaymentInputs::aggregate_keys(&[(op, InputKey::Taproot(odd))]).unwrap();
        let obs = ObservedInputs::aggregate(&[(op, InputKey::Taproot(odd).public_key())]).unwrap();
        assert_eq!(agg.a_pubkey, obs.a_pubkey);
        assert_eq!(agg.input_hash, obs.input_hash);
    }

    /// ELIP `test_pegin_input_excluded_from_shared_secret`.
    #[test]
    fn pegin_outpoint_participates_in_outpoint_l_but_contributes_no_key() {
        let eligible = [
            (
                Data::outpoint(0x61, 0),
                InputKey::Plain(Data::secret_key(0x51)),
            ),
            (
                Data::outpoint(0x62, 1),
                InputKey::Plain(Data::secret_key(0x52)),
            ),
        ];
        // Smaller than every eligible outpoint, so it decides outpoint_L.
        let pegin = Data::outpoint(0x01, 0);

        let without = SilentPaymentInputs::aggregate_keys(&eligible).unwrap();
        let with =
            SilentPaymentInputs::aggregate_with_extra_outpoints(&eligible, &[pegin]).unwrap();

        assert_eq!(with.a_sum, without.a_sum);
        assert_eq!(with.a_pubkey, without.a_pubkey);

        assert_ne!(
            with.input_hash, without.input_hash,
            "a peg-in outpoint smaller than every eligible one must change outpoint_L"
        );
        let expected = InputHasher::hash(&InputHasher::serialize_outpoint(&pegin), &with.a_pubkey);
        assert_eq!(with.input_hash, expected);

        let observed: Vec<_> = eligible.iter().map(|(o, k)| (*o, k.public_key())).collect();
        let obs = ObservedInputs::aggregate_with_extra_outpoints(&observed, &[pegin]).unwrap();
        assert_eq!(obs.input_hash, with.input_hash);
        assert_eq!(obs.a_pubkey, with.a_pubkey);
    }

    #[test]
    fn input_aggregation_rejects_empty() {
        assert!(matches!(
            SilentPaymentInputs::aggregate(&[]),
            Err(SilentPaymentInputError::NoInputs)
        ));
    }
}
