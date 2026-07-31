//! Scan-only material and labeled silent-payment addresses.

use crate::hashes::{Hash, HashEngine};
use crate::secp256k1::Scalar;
use crate::silentpayments::tags::LabelHash;
use crate::silentpayments::SilentPaymentAddress;
use crate::util::EC;

/// Scan-only silent-payment material.
pub use lwk_common::silentpayments::SilentPaymentScanMaterial;

/// The account coordinates a wallet's silent payments belong to.
pub use lwk_common::silentpayments::SilentPaymentAccount;

/// BIP-352 change label (`m = 0`).
pub const CHANGE_LABEL: u32 = 0;

/// Scan-material address derivation.
pub trait SilentPaymentScan {
    /// The (unlabeled) public address for this material.
    fn address(&self) -> SilentPaymentAddress;

    /// Derives BIP-352's label tweak for `m`.
    fn label_tweak(&self, m: u32) -> Scalar;

    /// Derives the labeled address for `m`.
    fn labeled_address(&self, m: u32) -> SilentPaymentAddress;

    /// The labeled spend base `B_m = B_spend + label_tweak_m·G`.
    fn labeled_spend_base(&self, m: u32) -> crate::secp256k1::PublicKey;
}

impl SilentPaymentScan for SilentPaymentScanMaterial {
    fn address(&self) -> SilentPaymentAddress {
        SilentPaymentAddress {
            scan: self.scan_seckey().public_key(&EC),
            spend: self.spend_pubkey(),
        }
    }

    fn label_tweak(&self, m: u32) -> Scalar {
        let mut eng = LabelHash::engine();
        eng.input(&self.scan_seckey().secret_bytes());
        eng.input(&m.to_be_bytes());
        let h = LabelHash::from_engine(eng);
        Scalar::from_be_bytes(h.to_byte_array()).expect("label tweak within curve order")
    }

    fn labeled_address(&self, m: u32) -> SilentPaymentAddress {
        SilentPaymentAddress {
            scan: self.scan_seckey().public_key(&EC),
            spend: self.labeled_spend_base(m),
        }
    }

    fn labeled_spend_base(&self, m: u32) -> crate::secp256k1::PublicKey {
        self.spend_pubkey()
            .add_exp_tweak(&EC, &self.label_tweak(m))
            .expect("labeled spend key")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;

    /// Addresses derive from scan material without a spend secret.
    #[test]
    fn addresses_derive_from_public_material_only() {
        let m = Data::material(0x11, 0x22);
        let plain = m.address();
        assert_eq!(plain.scan, m.scan_seckey().public_key(&EC));
        assert_eq!(plain.spend, m.spend_pubkey());

        for label in [CHANGE_LABEL, 7, 99] {
            let labeled = m.labeled_address(label);
            assert_eq!(labeled.scan, plain.scan, "scan key is never tweaked");
            assert_ne!(labeled.spend, plain.spend, "spend base is tweaked");
            assert_eq!(
                labeled.spend,
                plain
                    .spend
                    .add_exp_tweak(&EC, &m.label_tweak(label))
                    .unwrap(),
                "labeled base must be B_spend + label_tweak*G"
            );
        }
    }

    /// Labels commit to the scan key.
    #[test]
    fn label_tweak_commits_to_the_scan_key() {
        let a = Data::material(0x11, 0x22);
        let b = Data::material(0x33, 0x22);
        assert_eq!(a.spend_pubkey(), b.spend_pubkey(), "same spend base");
        assert_ne!(
            a.label_tweak(0).to_be_bytes(),
            b.label_tweak(0).to_be_bytes(),
            "different scan keys must give different label tweaks"
        );
    }

    /// Scan material exposes no spend secret.
    #[test]
    fn scan_material_has_no_spend_secret() {
        let m = Data::material(0x11, 0x22);

        let _: crate::secp256k1::PublicKey = m.spend_pubkey();

        let _: crate::secp256k1::SecretKey = m.scan_seckey();
    }
}
