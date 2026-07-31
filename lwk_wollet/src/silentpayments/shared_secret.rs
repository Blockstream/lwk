//! Shared-secret and per-output silent-payment derivation.

use crate::hashes::{Hash, HashEngine};
use crate::secp256k1::{PublicKey, Scalar, SecretKey};
use crate::silentpayments::tags::{BlindHash, SharedSecretHash};
use crate::silentpayments::{SilentPaymentInputs, SilentPaymentOutput};
use crate::util::EC;

/// An ECDH shared secret for one transaction and receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSecret(PublicKey);

impl SharedSecret {
    /// Sender's ECDH shared secret `S = input_hash · a · B_scan` from aggregated inputs.
    pub fn for_sender(scan_pubkey: &PublicKey, inputs: &SilentPaymentInputs) -> Self {
        let a_ih = inputs
            .a_sum
            .mul_tweak(&inputs.input_hash)
            .expect("scalar mul");
        SharedSecret(
            scan_pubkey
                .mul_tweak(
                    &EC,
                    &Scalar::from_be_bytes(a_ih.secret_bytes()).expect("scalar"),
                )
                .expect("ecdh point mul"),
        )
    }

    /// Receiver's ECDH shared secret `S = input_hash · b_scan · A`.
    pub fn for_receiver(scan_seckey: &SecretKey, a_sum_pubkey: &PublicKey, ih: &Scalar) -> Self {
        let bscan_ih = scan_seckey.mul_tweak(ih).expect("scalar mul");
        SharedSecret(
            a_sum_pubkey
                .mul_tweak(
                    &EC,
                    &Scalar::from_be_bytes(bscan_ih.secret_bytes()).expect("scalar"),
                )
                .expect("ecdh point mul"),
        )
    }

    /// Derives `S = b_scan · T` from a server partial tweak.
    pub fn from_partial_tweak(scan_seckey: &SecretKey, partial_tweak: &PublicKey) -> Self {
        SharedSecret(
            partial_tweak
                .mul_tweak(
                    &EC,
                    &Scalar::from_be_bytes(scan_seckey.secret_bytes()).expect("scalar"),
                )
                .expect("ecdh point mul"),
        )
    }

    /// The underlying point, `serP(S)` being what the tagged hashes commit to.
    pub fn as_pubkey(&self) -> &PublicKey {
        &self.0
    }

    /// `t_k = H_BIP0352/SharedSecret(serP(S) || ser32(k))`, returned as a tweak scalar.
    pub fn spend_tweak(&self, k: u32) -> Scalar {
        let mut eng = SharedSecretHash::engine();
        eng.input(&self.0.serialize());
        eng.input(&k.to_be_bytes());
        let h = SharedSecretHash::from_engine(eng);
        Scalar::from_be_bytes(h.to_byte_array()).expect("shared secret tweak within curve order")
    }

    /// `bk_k = H_LiquidSilentPayments/Blind(serP(S) || ser32(k))`.
    pub fn blinding_key(&self, k: u32) -> SecretKey {
        let mut eng = BlindHash::engine();
        eng.input(&self.0.serialize());
        eng.input(&k.to_be_bytes());
        let h = BlindHash::from_engine(eng);
        SecretKey::from_slice(&h.to_byte_array()).expect("blinding key within curve order")
    }

    /// Derives the output spend and blinding keys for `k`.
    pub fn derive_output(&self, spend_base: &PublicKey, k: u32) -> SilentPaymentOutput {
        let t_k = self.spend_tweak(k);
        let spend_pubkey = spend_base.add_exp_tweak(&EC, &t_k).expect("add exp tweak");

        let blinding_seckey = self.blinding_key(k);
        let blinding_pubkey = blinding_seckey.public_key(&EC);

        SilentPaymentOutput {
            spend_pubkey,
            blinding_pubkey,
            blinding_seckey,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::SilentPaymentScan;

    /// Q3: the spend tweak `t_k` and the blinding key `bk_k` are derived from the
    /// SAME shared secret `S` but in DIFFERENT tagged-hash domains, so they are
    /// independent — knowing one reveals nothing about the other, and the blinding
    /// key is never accidentally equal to (a function of) the spend tweak.
    #[test]
    fn blinding_key_domain_separated_from_spend_tweak() {
        let keys = Data::material(0x11, 0x22);
        let a_sum = Data::secret_key(0x33);
        let inputs = [(Data::outpoint(0xAB, 0), a_sum)];
        let agg = SilentPaymentInputs::aggregate(&inputs).unwrap();
        let s = SharedSecret::for_sender(&keys.address().scan, &agg);

        for k in 0..4u32 {
            let t_k = s.spend_tweak(k);
            let bk_k = s.blinding_key(k);
            // Distinct domains → distinct 32-byte values.
            assert_ne!(
                t_k.to_be_bytes(),
                bk_k.secret_bytes(),
                "spend tweak and blinding key must differ at k={k}"
            );
        }
    }

    /// The three routes to `S` — sender, receiver, and tweak-server partial tweak —
    /// must all land on the same point, or the scheme simply does not work.
    #[test]
    fn all_shared_secret_paths_agree() {
        let keys = Data::material(0x11, 0x22);
        let inputs = [
            (Data::outpoint(0x10, 0), Data::secret_key(0x31)),
            (Data::outpoint(0x20, 1), Data::secret_key(0x32)),
        ];
        let agg = SilentPaymentInputs::aggregate(&inputs).unwrap();

        let sender = SharedSecret::for_sender(&keys.address().scan, &agg);
        let receiver =
            SharedSecret::for_receiver(&keys.scan_seckey(), &agg.a_pubkey, &agg.input_hash);
        let via_server = SharedSecret::from_partial_tweak(
            &keys.scan_seckey(),
            crate::silentpayments::PartialTweak::new(&agg.a_pubkey, &agg.input_hash).as_pubkey(),
        );

        assert_eq!(sender, receiver);
        assert_eq!(sender, via_server);
    }
}
