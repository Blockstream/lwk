//! Silent Payments (BIP-352) on Liquid, per the Liquid silent payments ELIP.
//!
//! This crate holds `b_scan`, the public `B_spend`, and per-output [`SpendTweak`]s:
//! enough to detect, unblind, and track a silent payment, never enough to sign one.
//! A tweak is verified publicly as `B_spend + spend_tweak·G == spend pubkey`.
//!
//! Silent-payment scripts are derived from transaction inputs, not wallet descriptors.

pub mod address;
pub mod block_tweaks;
pub mod cache_entry;
pub mod inputs;
pub mod output;
mod pset_annotate;
pub mod receiver;
pub mod recipient;
pub mod scan_material;
pub mod scanner;
pub mod sender;
pub mod shared_secret;
pub mod sync;
mod tags;
#[cfg(test)]
pub(crate) mod test_fixture;
pub mod tweak_server;
pub mod tx_inputs;
pub mod tx_scan;
pub mod txout;
pub mod utxo;

pub use address::{SilentPaymentAddress, SilentPaymentAddressError};
pub use block_tweaks::BlockTweaks;
pub use cache_entry::SilentPaymentCacheEntry;
pub use inputs::{
    InputKey, InputKeyResult, MapInputProvider, ObservedInputs, SilentPaymentInputError,
    SilentPaymentInputProvider, SilentPaymentInputs,
};
pub use output::SilentPaymentOutput;
pub(crate) use pset_annotate::SilentPaymentPsetAnnotator;
pub use receiver::{SilentPaymentReceiver, SpendTweak};
pub use recipient::{ResolvedSilentPayment, SilentPaymentRecipient};
pub use scan_material::{
    SilentPaymentAccount, SilentPaymentScan, SilentPaymentScanMaterial, CHANGE_LABEL,
};
pub use scanner::{LabeledHit, SilentPaymentScanner};
pub use sender::SilentPaymentSender;
pub use shared_secret::SharedSecret;
pub use sync::SilentPaymentSync;
pub use tweak_server::{PartialTweak, SilentPaymentTweakClient};
pub use tx_inputs::{InputPubkeyRecovery, SilentPaymentTxInputs};
pub use tx_scan::SilentPaymentTxScanner;
pub use txout::SpTxOutBuilder;
pub use utxo::SilentPaymentUtxo;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;

    /// Test-only: production wallet code never holds `b_spend`.
    fn reconstruct_spend_key(
        b_spend: &crate::secp256k1::SecretKey,
        tweak: &SpendTweak,
    ) -> crate::secp256k1::SecretKey {
        b_spend
            .add_tweak(tweak.as_scalar())
            .expect("test vectors are in range")
    }

    /// The ELIP's known-answer vectors, pinned so an independent implementation can
    /// confirm cross-implementation agreement.
    #[test]
    fn known_answer_vectors() {
        use crate::elements::hashes::hex::DisplayHex;

        let b_spend = Data::secret_key(0x22);
        let keys = Data::material(0x11, 0x22);
        let inputs = [
            (Data::outpoint(0x10, 0), Data::secret_key(0x31)),
            (Data::outpoint(0x20, 1), Data::secret_key(0x32)),
        ];
        let sender = SilentPaymentSender::from_inputs(&inputs).unwrap();
        let agg = *sender.inputs();
        let receiver = SilentPaymentReceiver::new(keys);

        assert_eq!(
            agg.a_pubkey.serialize().to_lower_hex_string(),
            "031195a8046dcbb8e17034bca630065e7a0982e4e36f6f7e5a8d4554e4846fcd99",
            "A = a·G"
        );
        assert_eq!(
            agg.input_hash.to_be_bytes().to_lower_hex_string(),
            "d392922c00280a7e8d282182f5026f2fddbc74c1e1de18b4822128b2b77ec641",
            "input_hash"
        );

        // (k, P_spend, BK, bk, spend_sk, scriptPubKey)
        let expected: [(u32, &str, &str, &str, &str, &str); 2] = [
            (
                0,
                "02a29d9716417c964ca9e477343e71ffe730a4991a3eaad668eabec84e9feb7931",
                "0344e1289497e6da66fde710d2f38de053fc07355e405524401d7d609df5a1a8cc",
                "70ab8897b64bd21b427339ff4d014b883191ef6425862246c53bfc27a59aa3f0",
                "f03c436d2cd67ae1fecf7d88a38aa3a03c0abea43feaf6da8eb71e2e3a866bda",
                "5120a29d9716417c964ca9e477343e71ffe730a4991a3eaad668eabec84e9feb7931",
            ),
            (
                1,
                "0229d77654023af267dbe9cb7ff1956f947c816f203494381308387168fb010c92",
                "03efdeda770ccdbe8bf466fba48bfd2b2c436ab0c04658fc6d6c277de5078129fa",
                "945ba73a9804f62089c7d2ffdc079031031f0aebab372cec17ef9c110ebceb10",
                "9eff3472230fc83ef5ea8f8c80401c4eecd595a048bd2482a107d3a49baa5a58",
                "512029d77654023af267dbe9cb7ff1956f947c816f203494381308387168fb010c92",
            ),
        ];

        for (k, p_spend, bk_pub, bk_sec, spend_sk_hex, script_hex) in expected {
            let out = sender.derive_output(&keys.address(), k);
            let (recv_out, spend_tweak) = receiver.derive_output_from_observed(&agg.observed(), k);
            assert_eq!(out, recv_out);

            assert_eq!(
                out.spend_pubkey.serialize().to_lower_hex_string(),
                p_spend,
                "P_spend k={k}"
            );
            assert_eq!(
                out.blinding_pubkey.serialize().to_lower_hex_string(),
                bk_pub,
                "BK k={k}"
            );
            assert_eq!(
                out.blinding_seckey.secret_bytes().to_lower_hex_string(),
                bk_sec,
                "bk k={k}"
            );
            assert_eq!(
                reconstruct_spend_key(&b_spend, &spend_tweak)
                    .secret_bytes()
                    .to_lower_hex_string(),
                spend_sk_hex,
                "b_spend + t_k k={k}"
            );
            assert_eq!(
                spend_tweak.applied_to(&keys.spend_pubkey()).unwrap(),
                out.spend_pubkey,
                "B_spend + t_k*G k={k}"
            );
            assert_eq!(
                out.script_pubkey().as_bytes().to_lower_hex_string(),
                script_hex,
                "scriptPubKey k={k}"
            );
        }

        assert_eq!(
            keys.address().encode(crate::Network::Liquid),
            "lqsp1qqd8n2k7uklxq4aegau7vawtptkgxsja4kt99lpv6krctwpq8tpc65qjxd4lu4etruh9sngx3su9mtqp5fqzxz7re59y5nnez9p03ht3lyudcfhfe",
        );
    }
}
