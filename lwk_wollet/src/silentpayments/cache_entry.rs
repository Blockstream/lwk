//! Persisted wallet state for a discovered silent-payment output.

use crate::elements::{OutPoint, Script};
use crate::secp256k1::PublicKey;
use crate::silentpayments::{SpendTweak, CHANGE_LABEL};
use crate::Chain;

/// A discovered silent-payment output in wallet cache form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilentPaymentCacheEntry {
    /// Where the output sits on chain.
    pub outpoint: OutPoint,

    /// The output's scriptPubKey, `OP_1 <x_only(P_k)>`.
    pub script_pubkey: Script,

    /// The output counter `k` the scan found this at.
    pub k: u32,

    /// The BIP-352 label the payment was sent to, if any.
    pub label: Option<u32>,

    /// `t_k (+ label_tweak_m)` — the scalar that turns a signer's `b_spend` into
    /// this output's spend key. Not sufficient to spend by itself.
    pub spend_tweak: SpendTweak,

    /// `BK_k`, the output's blinding pubkey, kept so a confidential address can be
    /// rendered without re-deriving the shared secret.
    pub blinding_pubkey: PublicKey,
}

impl SilentPaymentCacheEntry {
    /// Classifies change-labeled outputs as internal.
    pub fn chain(&self) -> Chain {
        if self.label == Some(CHANGE_LABEL) {
            Chain::Internal
        } else {
            Chain::External
        }
    }

    /// Whether this output is the wallet's own silent-payment change.
    pub fn is_change(&self) -> bool {
        self.label == Some(CHANGE_LABEL)
    }

    /// Checks `B_spend + spend_tweak·G == x_only(P_k)`.
    pub fn verify(&self, spend_base: &PublicKey) -> bool {
        let Some(expected) = self.spend_tweak.applied_to(spend_base) else {
            return false;
        };
        self.x_only_pubkey() == Some(expected.x_only_public_key().0)
    }

    /// Extracts `x_only(P_k)` from the stored script.
    pub fn x_only_pubkey(&self) -> Option<crate::elements::secp256k1_zkp::XOnlyPublicKey> {
        let bytes = self.script_pubkey.as_bytes();
        // `OP_1 <32-byte push>`: 0x51 0x20 followed by the key.
        if bytes.len() != 34 || bytes[0] != 0x51 || bytes[1] != 0x20 {
            return None;
        }
        crate::elements::secp256k1_zkp::XOnlyPublicKey::from_slice(&bytes[2..]).ok()
    }

    /// Weight of a key-path Taproot satisfaction.
    pub fn max_weight_to_satisfy(&self) -> usize {
        crate::silentpayments::SilentPaymentUtxo::MAX_WEIGHT_TO_SATISFY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::{
        SilentPaymentReceiver, SilentPaymentScanMaterial, SilentPaymentSender,
    };

    fn entry_and_material() -> (SilentPaymentCacheEntry, SilentPaymentScanMaterial) {
        let m = Data::material(0x11, 0x22);
        let inputs = [
            (Data::outpoint(1, 0), Data::secret_key(0xA1)),
            (Data::outpoint(2, 1), Data::secret_key(0xA2)),
        ];
        let sender = SilentPaymentSender::from_inputs(&inputs).unwrap();
        let observed = sender.inputs().observed();
        let (output, spend_tweak) =
            SilentPaymentReceiver::new(m).derive_output_from_observed(&observed, 0);

        let entry = SilentPaymentCacheEntry {
            outpoint: Data::outpoint(9, 0),
            script_pubkey: output.script_pubkey(),
            k: 0,
            label: None,
            spend_tweak,
            blinding_pubkey: output.blinding_pubkey,
        };
        (entry, m)
    }

    #[test]
    fn entry_verifies_only_against_its_own_material() {
        let (entry, m) = entry_and_material();
        assert!(
            entry.verify(&m.spend_pubkey()),
            "the stored tweak must verify"
        );

        let stranger = Data::material(0x77, 0x88);
        assert!(
            !entry.verify(&stranger.spend_pubkey()),
            "a different spend base must not verify this entry"
        );

        let mut corrupted = entry;
        corrupted.spend_tweak = SpendTweak::from_be_bytes([0x5A; 32]).unwrap();
        assert!(
            !corrupted.verify(&m.spend_pubkey()),
            "a corrupted tweak must fail rather than be trusted"
        );
    }

    #[test]
    fn change_label_maps_to_the_internal_chain() {
        let (mut entry, _) = entry_and_material();
        assert_eq!(entry.chain(), Chain::External);
        assert!(!entry.is_change());

        entry.label = Some(CHANGE_LABEL);
        assert_eq!(entry.chain(), Chain::Internal);
        assert!(entry.is_change());

        entry.label = Some(7);
        assert_eq!(entry.chain(), Chain::External);
        assert!(!entry.is_change());
    }

    #[test]
    fn entry_is_read_as_taproot_or_not_at_all() {
        let (entry, _) = entry_and_material();
        assert_eq!(
            entry.max_weight_to_satisfy(),
            crate::silentpayments::SilentPaymentUtxo::MAX_WEIGHT_TO_SATISFY
        );

        let mut not_taproot = entry;
        not_taproot.script_pubkey = Script::from(vec![0x00, 0x14, 0xAB, 0xCD]);
        assert!(not_taproot.x_only_pubkey().is_none());
    }
}
