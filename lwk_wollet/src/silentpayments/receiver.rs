//! Recomputes silent-payment outputs and spend tweaks.

use crate::secp256k1::{PublicKey, Scalar};
use crate::silentpayments::inputs::InputHasher;
use crate::silentpayments::{
    ObservedInputs, SharedSecret, SilentPaymentOutput, SilentPaymentScan, SilentPaymentScanMaterial,
};

/// Scalar that tweaks `b_spend` into an output spend key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpendTweak(Scalar);

impl SpendTweak {
    /// Wrap a raw scalar as a spend tweak.
    pub fn from_scalar(scalar: Scalar) -> Self {
        SpendTweak(scalar)
    }

    /// Underlying scalar.
    pub fn as_scalar(&self) -> &Scalar {
        &self.0
    }

    /// The 32 big-endian bytes, as persisted and as carried in PSET metadata.
    pub fn to_be_bytes(self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    /// Rebuild from persisted bytes.
    pub fn from_be_bytes(bytes: [u8; 32]) -> Option<Self> {
        Scalar::from_be_bytes(bytes).ok().map(SpendTweak)
    }

    /// Adds a label tweak.
    pub fn add_label_tweak(self, label_tweak: &Scalar) -> Option<Self> {
        let combined = crate::secp256k1::SecretKey::from_slice(&self.0.to_be_bytes())
            .ok()?
            .add_tweak(label_tweak)
            .ok()?;
        Scalar::from_be_bytes(combined.secret_bytes())
            .ok()
            .map(SpendTweak)
    }

    /// Applies this tweak to a spend base.
    pub fn applied_to(&self, spend_base: &PublicKey) -> Option<PublicKey> {
        spend_base.add_exp_tweak(&crate::util::EC, &self.0).ok()
    }
}

/// Recomputes outputs from scan material.
#[derive(Debug, Clone, Copy)]
pub struct SilentPaymentReceiver {
    material: SilentPaymentScanMaterial,
}

impl SilentPaymentReceiver {
    /// Build a receiver from the wallet's scan-only material.
    pub fn new(material: SilentPaymentScanMaterial) -> Self {
        SilentPaymentReceiver { material }
    }

    /// The scan-only material backing this receiver.
    pub fn material(&self) -> &SilentPaymentScanMaterial {
        &self.material
    }

    /// Recomputes output `k` from aggregated inputs.
    pub fn derive_output(
        &self,
        a_sum_pubkey: &PublicKey,
        input_hash: &Scalar,
        k: u32,
    ) -> (SilentPaymentOutput, SpendTweak) {
        let shared_secret =
            SharedSecret::for_receiver(&self.material.scan_seckey(), a_sum_pubkey, input_hash);
        self.derive_from_shared_secret(&shared_secret, k)
    }

    /// Recompute the output for index `k` from an observer's aggregated inputs.
    pub fn derive_output_from_observed(
        &self,
        observed: &ObservedInputs,
        k: u32,
    ) -> (SilentPaymentOutput, SpendTweak) {
        self.derive_output(&observed.a_pubkey, &observed.input_hash, k)
    }

    /// Recomputes output `k` from an aggregate pubkey and raw outpoint.
    pub fn derive_output_from_raw(
        &self,
        a_sum_pubkey: &PublicKey,
        outpoint_l: &[u8],
        k: u32,
    ) -> (SilentPaymentOutput, SpendTweak) {
        let ih = InputHasher::hash(outpoint_l, a_sum_pubkey);
        self.derive_output(a_sum_pubkey, &ih, k)
    }

    /// Derives output `k` from a shared secret.
    pub(crate) fn derive_from_shared_secret(
        &self,
        shared_secret: &SharedSecret,
        k: u32,
    ) -> (SilentPaymentOutput, SpendTweak) {
        let out = shared_secret.derive_output(&self.material.spend_pubkey(), k);
        (out, SpendTweak::from_scalar(shared_secret.spend_tweak(k)))
    }

    /// The labeled spend base `B_m` for label `m`, computed by public point addition.
    pub(crate) fn labeled_spend_base(&self, m: u32) -> PublicKey {
        self.material.labeled_spend_base(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::SilentPaymentSender;

    fn material() -> SilentPaymentScanMaterial {
        Data::material(0x11, 0x22)
    }

    /// Spend tweaks reproduce output keys from public data.
    #[test]
    fn spend_tweak_reproduces_the_output_key_from_public_data() {
        let m = material();
        let inputs = [
            (Data::outpoint(1, 0), Data::secret_key(0xA1)),
            (Data::outpoint(2, 1), Data::secret_key(0xA2)),
        ];
        let sender = SilentPaymentSender::from_inputs(&inputs).unwrap();
        let receiver = SilentPaymentReceiver::new(m);

        for k in 0..3u32 {
            let (out, tweak) = receiver.derive_output_from_observed(&sender.inputs().observed(), k);
            assert_eq!(
                tweak.applied_to(&m.spend_pubkey()).unwrap(),
                out.spend_pubkey,
                "B_spend + t_k*G must equal the output spend key at k={k}"
            );
            assert_eq!(
                SpendTweak::from_be_bytes(tweak.to_be_bytes()),
                Some(tweak),
                "a persisted tweak must come back unchanged"
            );
        }

        let base_tweak = SpendTweak::from_scalar(Scalar::from_be_bytes([0x09; 32]).unwrap());
        let label = 7u32;
        let combined = base_tweak.add_label_tweak(&m.label_tweak(label)).unwrap();
        assert_eq!(
            combined.applied_to(&m.spend_pubkey()).unwrap(),
            base_tweak.applied_to(&m.labeled_spend_base(label)).unwrap(),
            "folding a label in must equal tweaking the labeled base"
        );
    }
}
