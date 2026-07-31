//! Public silent-payment input-key recovery.

use crate::elements::{OutPoint, Script, Transaction};
use crate::secp256k1::PublicKey;
use crate::silentpayments::{ObservedInputs, SilentPaymentInputError};

/// Recovers the single pubkey one input contributes to `A = Σ A_i`.
pub struct InputPubkeyRecovery<'a> {
    prevout_script: &'a Script,
    script_sig: &'a Script,
    witness: &'a [Vec<u8>],
}

impl<'a> InputPubkeyRecovery<'a> {
    const COMPRESSED_LEN: usize = 33;
    const P2SH_P2WPKH_REDEEM_LEN: usize = 22;
    const ANNEX_PREFIX: u8 = 0x50;

    /// Build a recovery view over one input.
    pub fn new(prevout_script: &'a Script, script_sig: &'a Script, witness: &'a [Vec<u8>]) -> Self {
        InputPubkeyRecovery {
            prevout_script,
            script_sig,
            witness,
        }
    }

    /// Recovers this input's eligible pubkey.
    pub fn recover(&self) -> Option<PublicKey> {
        if self.prevout_script.is_v1_p2tr() {
            self.taproot_output_key()
        } else if self.prevout_script.is_v0_p2wpkh() {
            self.witness_pubkey()
        } else if self.prevout_script.is_p2sh() {
            self.nested_p2wpkh_pubkey()
        } else if self.prevout_script.is_p2pkh() {
            self.script_sig_pubkey()
        } else {
            None
        }
    }

    /// The NUMS point `H` from BIP-341, `lift_x(0x50929b74...803ac)`.
    const NUMS_H: [u8; 32] = [
        0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a,
        0x5e, 0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80,
        0x3a, 0xc0,
    ];

    /// Recovers an eligible Taproot output key.
    fn taproot_output_key(&self) -> Option<PublicKey> {
        if !self.is_eligible_taproot_spend() {
            return None;
        }

        let spk = self.prevout_script.as_bytes();
        let x_only = spk.get(2..34)?;
        let key = crate::elements::secp256k1_zkp::XOnlyPublicKey::from_slice(x_only).ok()?;
        Some(PublicKey::from_x_only_public_key(
            key,
            crate::elements::secp256k1_zkp::Parity::Even,
        ))
    }

    fn is_eligible_taproot_spend(&self) -> bool {
        let mut stack = self.witness;
        // A single witness item is always the key-path signature, never an annex.
        if stack.len() > 1 {
            if let Some((last, rest)) = stack.split_last() {
                if last.first() == Some(&Self::ANNEX_PREFIX) {
                    stack = rest;
                }
            }
        }

        match stack.len() {
            1 => true,
            n if n >= 2 => {
                let control_block = &stack[n - 1];
                match crate::elements::taproot::ControlBlock::from_slice(control_block) {
                    Ok(cb) => cb.internal_key.serialize() != Self::NUMS_H,
                    Err(_) => false,
                }
            }
            _ => false,
        }
    }

    /// P2WPKH: witness is `[signature, pubkey]`.
    fn witness_pubkey(&self) -> Option<PublicKey> {
        if self.witness.len() != 2 {
            return None;
        }
        Self::parse_pubkey(self.witness.get(1)?)
    }

    /// Recovers a P2SH-wrapped P2WPKH pubkey.
    fn nested_p2wpkh_pubkey(&self) -> Option<PublicKey> {
        let sig = self.script_sig.as_bytes();
        let redeem = sig.strip_prefix(&[Self::P2SH_P2WPKH_REDEEM_LEN as u8])?;
        if redeem.len() != Self::P2SH_P2WPKH_REDEEM_LEN || redeem[0] != 0x00 || redeem[1] != 0x14 {
            return None;
        }
        self.witness_pubkey()
    }

    /// Recovers the committed compressed P2PKH pubkey.
    fn script_sig_pubkey(&self) -> Option<PublicKey> {
        let spk_hash = self.prevout_script.as_bytes().get(3..3 + 20)?;
        let sig = self.script_sig.as_bytes();

        (Self::COMPRESSED_LEN..=sig.len()).rev().find_map(|end| {
            let candidate = &sig[end - Self::COMPRESSED_LEN..end];
            use crate::elements::hashes::{hash160, Hash as _};
            (hash160::Hash::hash(candidate).to_byte_array() == spk_hash)
                .then(|| Self::parse_pubkey(candidate))
                .flatten()
        })
    }

    /// Parses a compressed pubkey.
    fn parse_pubkey(bytes: &[u8]) -> Option<PublicKey> {
        if bytes.len() != Self::COMPRESSED_LEN {
            return None;
        }
        PublicKey::from_slice(bytes).ok()
    }
}

/// Transaction inputs classified for silent payments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SilentPaymentTxInputs {
    eligible: Vec<(OutPoint, PublicKey)>,
    ineligible: Vec<OutPoint>,
    unknown_segwit_version: bool,
}

impl SilentPaymentTxInputs {
    /// Classifies transaction inputs from their prevout scripts.
    pub fn extract<'a, F>(tx: &Transaction, mut prevout_script: F) -> Self
    where
        F: FnMut(&OutPoint) -> Option<&'a Script>,
    {
        let mut eligible = Vec::new();
        let mut ineligible = Vec::new();
        let mut unknown_segwit_version = false;

        for input in &tx.input {
            let outpoint = input.previous_output;

            if input.is_pegin() {
                ineligible.push(outpoint);
                continue;
            }

            let spk = prevout_script(&outpoint);

            if spk.is_some_and(Self::is_unknown_segwit_version) {
                unknown_segwit_version = true;
            }

            let pubkey = spk.and_then(|spk| {
                InputPubkeyRecovery::new(spk, &input.script_sig, &input.witness.script_witness)
                    .recover()
            });

            match pubkey {
                Some(pk) => eligible.push((outpoint, pk)),
                None => ineligible.push(outpoint),
            }
        }

        SilentPaymentTxInputs {
            eligible,
            ineligible,
            unknown_segwit_version,
        }
    }

    fn is_unknown_segwit_version(spk: &Script) -> bool {
        if !spk.is_witness_program() {
            return false;
        }
        let version_byte = spk.as_bytes()[0];
        let version = if version_byte == 0 {
            0
        } else {
            version_byte.wrapping_sub(0x50)
        };
        version > 1
    }

    /// The inputs contributing to `A`, as `(outpoint, pubkey)` pairs.
    pub fn eligible(&self) -> &[(OutPoint, PublicKey)] {
        &self.eligible
    }

    /// The outpoints of inputs contributing no key but still entering `outpoint_L`.
    pub fn ineligible(&self) -> &[OutPoint] {
        &self.ineligible
    }

    /// Whether this transaction could carry a silent payment at all.
    pub fn is_eligible(&self) -> bool {
        !self.eligible.is_empty() && !self.unknown_segwit_version
    }

    /// Whether an input uses an unsupported SegWit version.
    pub fn must_not_be_scanned(&self) -> bool {
        self.unknown_segwit_version
    }

    /// Aggregates the observer-side input data.
    pub fn observed(&self) -> Result<ObservedInputs, SilentPaymentInputError> {
        if self.unknown_segwit_version {
            return Err(SilentPaymentInputError::NoInputs);
        }
        ObservedInputs::aggregate_with_extra_outpoints(&self.eligible, &self.ineligible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Transaction, TxIn};
    use crate::util::EC;
    use lwk_test_util::ElementsTestData as Data;

    /// Prevout scripts of each shape the recovery has to classify.
    ///
    /// Built through elements' own hash types: `elements` and `elements::bitcoin`
    /// have distinct `WPubkeyHash`/`PubkeyHash` newtypes over the same digest.
    struct Spk;

    impl Spk {
        fn p2wpkh(pk: &PublicKey) -> Script {
            Data::p2wpkh_of(pk)
        }

        fn p2pkh_of(key_bytes: &[u8]) -> Script {
            use crate::elements::hashes::{hash160, Hash as _};
            let hash = hash160::Hash::hash(key_bytes);
            Script::new_p2pkh(&crate::elements::PubkeyHash::from_byte_array(
                hash.to_byte_array(),
            ))
        }

        fn p2pkh(pk: &PublicKey) -> Script {
            Self::p2pkh_of(&pk.serialize())
        }

        fn p2tr(pk: &PublicKey) -> Script {
            let (x_only, _) = pk.x_only_public_key();
            Script::new_v1_p2tr_tweaked(crate::elements::schnorr::TweakedPublicKey::new(x_only))
        }

        fn p2sh(redeem: &Script) -> Script {
            Script::new_p2sh(&redeem.script_hash())
        }

        /// OP_2 PUSH32: a well-formed witness program of a version SP does not know.
        fn unknown_witness_version() -> Script {
            let mut bytes = vec![0x52u8, 0x20];
            bytes.extend_from_slice(&[0xAA; 32]);
            Script::from(bytes)
        }
    }

    struct ScriptSig;

    impl ScriptSig {
        fn push(data: &[u8]) -> Script {
            let mut bytes = vec![data.len() as u8];
            bytes.extend_from_slice(data);
            Script::from(bytes)
        }

        fn p2pkh(key_bytes: &[u8]) -> Script {
            let mut bytes = vec![71u8];
            bytes.extend_from_slice(&[0x30; 71]);
            bytes.push(key_bytes.len() as u8);
            bytes.extend_from_slice(key_bytes);
            Script::from(bytes)
        }

        fn p2pkh_with_trailing_push(key_bytes: &[u8], junk: &[u8]) -> Script {
            let mut bytes = Self::p2pkh(key_bytes).as_bytes().to_vec();
            bytes.push(junk.len() as u8);
            bytes.extend_from_slice(junk);
            Script::from(bytes)
        }
    }

    struct Witness;

    impl Witness {
        fn p2wpkh(pk: &PublicKey) -> Vec<Vec<u8>> {
            Data::p2wpkh_witness(pk)
        }

        fn keypath() -> Vec<Vec<u8>> {
            vec![vec![0x30; 64]]
        }

        /// Signature, one leaf script, then a control block: leaf-version/parity byte
        /// followed by the x-only internal key, with no merkle branch.
        fn script_path(internal_key: &crate::secp256k1::XOnlyPublicKey) -> Vec<Vec<u8>> {
            let mut control = vec![0xc0u8];
            control.extend_from_slice(&internal_key.serialize());
            vec![vec![0x30; 64], vec![0xab; 32], control]
        }
    }

    /// One prevout/spend shape and the key it must contribute, if any.
    struct RecoveryCase {
        why: &'static str,
        spk: Script,
        script_sig: Script,
        witness: Vec<Vec<u8>>,
        expected: Option<PublicKey>,
    }

    impl RecoveryCase {
        fn new(
            why: &'static str,
            spk: Script,
            script_sig: Script,
            witness: Vec<Vec<u8>>,
            expected: Option<PublicKey>,
        ) -> Self {
            Self {
                why,
                spk,
                script_sig,
                witness,
                expected,
            }
        }

        fn check(&self) {
            assert_eq!(
                InputPubkeyRecovery::new(&self.spk, &self.script_sig, &self.witness).recover(),
                self.expected,
                "{}",
                self.why
            );
        }
    }

    struct TxFixture;

    impl TxFixture {
        fn input(previous_output: OutPoint, script_witness: Vec<Vec<u8>>, is_pegin: bool) -> TxIn {
            Data::input(previous_output, script_witness, is_pegin)
        }

        fn of(input: Vec<TxIn>) -> Transaction {
            Transaction {
                version: 2,
                lock_time: crate::elements::LockTime::ZERO,
                input,
                output: vec![],
            }
        }
    }

    /// Taproot keys must come back in even-Y form even when the signer's key is
    /// odd-Y, because that is all an observer can see in the scriptPubKey. Getting
    /// this wrong makes half of all taproot inputs derive the wrong `A`.
    #[test]
    fn taproot_recovers_even_y_output_key() {
        // Find a key with odd Y so the negation path is actually exercised.
        let (secret, pk) = (1u8..40)
            .map(|b| (Data::secret_key(b), Data::secret_key(b).public_key(&EC)))
            .find(|(_, pk)| pk.x_only_public_key().1 == crate::elements::secp256k1_zkp::Parity::Odd)
            .expect("some seed yields an odd-Y key");
        let spk = Spk::p2tr(&pk);
        let witness = Witness::keypath();

        let got = InputPubkeyRecovery::new(&spk, &Script::new(), &witness)
            .recover()
            .expect("key-path taproot is eligible");

        assert_eq!(got, secret.negate().public_key(&EC));
        assert_ne!(got, pk, "the odd-Y key must not be summed as-is");
        assert_eq!(
            got,
            crate::silentpayments::InputKey::Taproot(secret).public_key()
        );
    }

    /// Every prevout shape the recovery has to classify, plus the malleations that
    /// must not change its answer. `None` means the input contributes no key —
    /// either the shape is not SP-eligible, or the key it commits to is not permitted.
    #[test]
    fn single_input_key_recovery() {
        let pubk = |b: u8| Data::secret_key(b).public_key(&EC);
        let taproot =
            |b: u8| crate::silentpayments::InputKey::Taproot(Data::secret_key(b)).public_key();

        let nums_h =
            crate::secp256k1::XOnlyPublicKey::from_slice(&InputPubkeyRecovery::NUMS_H).unwrap();
        let junk = pubk(0x99);
        let redeem = Spk::p2wpkh(&pubk(0x23));
        let not_p2wpkh = Script::from(vec![0x52, 0x53, 0x54]);
        let uncompressed = pubk(0x26).serialize_uncompressed();

        // A 64-byte BIP-340 signature begins with 0x50 about 1 time in 256, and
        // BIP-341 recognizes an annex only from 2 witness elements up.
        let sig_resembling_an_annex = {
            let mut sig = vec![0x50u8];
            sig.extend_from_slice(&[0xAB; 63]);
            vec![sig]
        };

        let cases = [
            RecoveryCase::new(
                "p2wpkh key is summed as-is",
                Spk::p2wpkh(&pubk(0x21)),
                Script::new(),
                Witness::p2wpkh(&pubk(0x21)),
                Some(pubk(0x21)),
            ),
            RecoveryCase::new(
                "script-path taproot contributes the output key, not the internal key",
                Spk::p2tr(&pubk(0x22)),
                Script::new(),
                Witness::script_path(&junk.x_only_public_key().0),
                Some(taproot(0x22)),
            ),
            RecoveryCase::new(
                "script-path taproot using NUMS H is not eligible",
                Spk::p2tr(&pubk(0x23)),
                Script::new(),
                Witness::script_path(&nums_h),
                None,
            ),
            RecoveryCase::new(
                "a one-element witness is a key-path spend, not an annex",
                Spk::p2tr(&pubk(0x25)),
                Script::new(),
                sig_resembling_an_annex,
                Some(taproot(0x25)),
            ),
            RecoveryCase::new(
                "malformed control block is not eligible",
                Spk::p2tr(&pubk(0x24)),
                Script::new(),
                vec![vec![0x30; 64], vec![0xab; 32], vec![0xc0; 10]],
                None,
            ),
            RecoveryCase::new(
                "p2sh is eligible when it wraps p2wpkh",
                Spk::p2sh(&redeem),
                ScriptSig::push(redeem.as_bytes()),
                Witness::p2wpkh(&pubk(0x23)),
                Some(pubk(0x23)),
            ),
            RecoveryCase::new(
                "p2sh wrapping anything else is not",
                Spk::p2sh(&redeem),
                ScriptSig::push(not_p2wpkh.as_bytes()),
                Witness::p2wpkh(&pubk(0x23)),
                None,
            ),
            RecoveryCase::new(
                "legacy p2pkh key matches the committed hash160",
                Spk::p2pkh(&pubk(0x24)),
                ScriptSig::p2pkh(&pubk(0x24).serialize()),
                vec![],
                Some(pubk(0x24)),
            ),
            RecoveryCase::new(
                "BIP-352 permits only compressed and x-only keys",
                Spk::p2pkh_of(&uncompressed),
                ScriptSig::p2pkh(&uncompressed),
                vec![],
                None,
            ),
            RecoveryCase::new(
                "p2pkh key is found by hash160, not by taking the last push",
                Spk::p2pkh(&pubk(0x27)),
                ScriptSig::p2pkh_with_trailing_push(&pubk(0x27).serialize(), &junk.serialize()),
                vec![],
                Some(pubk(0x27)),
            ),
        ];

        for case in &cases {
            case.check();
        }
    }

    #[test]
    fn extract_over_transaction_agrees_with_direct_aggregation() {
        let keys = [Data::secret_key(0x31), Data::secret_key(0x32)];
        let outpoints = [Data::outpoint(0x30, 1), Data::outpoint(0x10, 0)];
        let spks: Vec<Script> = keys
            .iter()
            .map(|k| Spk::p2wpkh(&k.public_key(&EC)))
            .collect();

        let tx = TxFixture::of(
            (0..2)
                .map(|i| {
                    TxFixture::input(
                        outpoints[i],
                        Witness::p2wpkh(&keys[i].public_key(&EC)),
                        false,
                    )
                })
                .collect(),
        );

        let lookup: Vec<(OutPoint, Script)> = outpoints.iter().copied().zip(spks).collect();
        let extracted = SilentPaymentTxInputs::extract(&tx, |o| {
            lookup.iter().find(|(op, _)| op == o).map(|(_, s)| s)
        });

        assert!(extracted.is_eligible());
        assert_eq!(extracted.eligible().len(), 2);
        assert!(extracted.ineligible().is_empty());

        let direct = crate::silentpayments::SilentPaymentInputs::aggregate(&[
            (outpoints[0], keys[0]),
            (outpoints[1], keys[1]),
        ])
        .unwrap();
        assert_eq!(extracted.observed().unwrap(), direct.observed());
    }

    #[test]
    fn unknown_prevout_is_ineligible_but_keeps_its_outpoint() {
        let key = Data::secret_key(0x41);
        let known = Data::outpoint(0x50, 0);
        // Smaller serialization than `known`, so it decides outpoint_L if counted.
        let unknown = Data::outpoint(0x10, 0);
        let spk = Spk::p2wpkh(&key.public_key(&EC));
        let witness = Witness::p2wpkh(&key.public_key(&EC));

        let tx = TxFixture::of(vec![
            TxFixture::input(known, witness.clone(), false),
            TxFixture::input(unknown, witness, false),
        ]);

        let extracted =
            SilentPaymentTxInputs::extract(&tx, |o| if *o == known { Some(&spk) } else { None });

        assert_eq!(extracted.eligible().len(), 1);
        assert_eq!(extracted.ineligible(), &[unknown]);

        let with = extracted.observed().unwrap();
        let without = ObservedInputs::aggregate(&[(known, key.public_key(&EC))]).unwrap();
        assert_eq!(with.a_pubkey, without.a_pubkey, "A only sums eligible keys");
        assert_ne!(
            with.input_hash, without.input_hash,
            "the ineligible outpoint must still affect outpoint_L"
        );
    }

    #[test]
    fn pegin_input_contributes_no_key() {
        let key = Data::secret_key(0x42);
        let op = Data::outpoint(0x60, 3);
        let spk = Spk::p2wpkh(&key.public_key(&EC));

        let tx = TxFixture::of(vec![TxFixture::input(
            op,
            Witness::p2wpkh(&key.public_key(&EC)),
            true,
        )]);

        let extracted = SilentPaymentTxInputs::extract(&tx, |_| Some(&spk));
        assert!(
            !extracted.is_eligible(),
            "a peg-in alone is not SP-eligible"
        );
        assert_eq!(extracted.ineligible(), &[op]);
        assert_eq!(extracted.observed(), Err(SilentPaymentInputError::NoInputs));
    }

    #[test]
    fn unknown_segwit_version_disqualifies_the_whole_transaction() {
        let key = Data::secret_key(0x51);
        let known = Data::outpoint(0x70, 0);
        let unknown_version = Data::outpoint(0x71, 0);
        let p2wpkh = Spk::p2wpkh(&key.public_key(&EC));
        let v2_program = Spk::unknown_witness_version();

        let tx = TxFixture::of(vec![
            TxFixture::input(known, Witness::p2wpkh(&key.public_key(&EC)), false),
            TxFixture::input(unknown_version, vec![vec![0xAB; 64]], false),
        ]);

        let lookup = |o: &OutPoint| {
            if *o == known {
                Some(&p2wpkh)
            } else if *o == unknown_version {
                Some(&v2_program)
            } else {
                None
            }
        };

        let extracted = SilentPaymentTxInputs::extract(&tx, lookup);
        assert!(extracted.must_not_be_scanned());
        assert!(
            !extracted.is_eligible(),
            "an unknown SegWit version input must veto the whole transaction, \
             even with an otherwise-eligible P2WPKH input present"
        );
        assert_eq!(
            extracted.observed(),
            Err(SilentPaymentInputError::NoInputs),
            "observed() must also refuse a transaction that must not be scanned"
        );
    }

    #[test]
    fn known_segwit_versions_do_not_trip_the_veto() {
        let key = Data::secret_key(0x52).public_key(&EC);
        assert!(!SilentPaymentTxInputs::is_unknown_segwit_version(
            &Spk::p2wpkh(&key)
        ));
        assert!(!SilentPaymentTxInputs::is_unknown_segwit_version(
            &Spk::p2tr(&key)
        ));
    }
}
