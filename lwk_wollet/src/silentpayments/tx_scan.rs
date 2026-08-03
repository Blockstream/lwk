//! Scans transactions for wallet-owned silent payments.

use crate::elements::{OutPoint, Script, Transaction};
use crate::silentpayments::{
    PartialTweak, SilentPaymentScanMaterial, SilentPaymentScanner, SilentPaymentTxInputs,
    SilentPaymentUtxo,
};
use crate::util::EC;

/// Scans complete transactions for wallet-owned silent payments.
#[derive(Debug, Clone)]
pub struct SilentPaymentTxScanner {
    scanner: SilentPaymentScanner,
}

impl SilentPaymentTxScanner {
    /// A scanner for `material`, detecting plain (unlabeled) payments only.
    pub fn new(material: SilentPaymentScanMaterial) -> Self {
        SilentPaymentTxScanner {
            scanner: SilentPaymentScanner::new(material),
        }
    }

    /// Also detects payments sent to `labels`.
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = u32>) -> Self {
        self.scanner = self.scanner.with_labels(labels);
        self
    }

    /// Stop scanning a transaction after `gap_limit` consecutive output indices miss.
    pub fn with_gap_limit(mut self, gap_limit: u32) -> Self {
        self.scanner = self.scanner.with_gap_limit(gap_limit);
        self
    }

    /// Finds wallet-owned outputs using input prevout scripts.
    pub fn scan_tx<'a, F>(&self, tx: &Transaction, prevout_script: F) -> Vec<SilentPaymentUtxo>
    where
        F: FnMut(&OutPoint) -> Option<&'a Script>,
    {
        let inputs = SilentPaymentTxInputs::extract(tx, prevout_script);

        if !inputs.is_eligible() {
            return Vec::new();
        }

        let Ok(observed) = inputs.observed() else {
            return Vec::new();
        };

        let tweak = PartialTweak::from_observed(&observed);
        self.scan_tx_with_tweak(tx, &tweak)
    }

    /// Scans `tx` using an externally obtained partial tweak.
    pub fn scan_tx_with_tweak(
        &self,
        tx: &Transaction,
        tweak: &PartialTweak,
    ) -> Vec<SilentPaymentUtxo> {
        let txid = tx.txid();
        let scripts: Vec<Script> = tx.output.iter().map(|o| o.script_pubkey.clone()).collect();

        let mut found = Vec::new();
        for hit in self.scanner.scan(tweak, &scripts) {
            let want = hit.output.script_pubkey();

            let Some(vout) = tx.output.iter().position(|o| o.script_pubkey == want) else {
                continue;
            };
            let txout = &tx.output[vout];

            let Ok(unblinded) = txout.unblind(&EC, hit.output.blinding_seckey) else {
                continue;
            };

            found.push(SilentPaymentUtxo::new(
                OutPoint::new(txid, vout as u32),
                txout.clone(),
                unblinded,
                hit,
            ));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::test_fixture::SpPaymentBuilder;
    use crate::silentpayments::{
        SilentPaymentScan, SilentPaymentSender, SpTxOutBuilder, CHANGE_LABEL,
    };

    fn keys() -> SilentPaymentScanMaterial {
        Data::material(0x11, 0x22)
    }

    #[test]
    fn scan_tx_finds_nothing_it_should_not() {
        let keys = keys();
        let inputs = [(Data::outpoint(0x22, 1), Data::secret_key(0xA1))];

        let stranger = Data::material(0x77, 0x88);
        let to_stranger = SpPaymentBuilder::new()
            .with_inputs(&inputs)
            .with_value(1_000)
            .build_for(&stranger);
        assert!(
            SilentPaymentTxScanner::new(keys)
                .scan_tx(&to_stranger.tx, to_stranger.prevout_lookup())
                .is_empty(),
            "must not claim another wallet's payment"
        );

        let to_us = SpPaymentBuilder::new()
            .with_inputs(&inputs)
            .build_for(&keys);
        assert!(
            SilentPaymentTxScanner::new(keys)
                .scan_tx(&to_us.tx, |_| None)
                .is_empty(),
            "without prevouts there is no A, so there must be no discovery"
        );
    }

    /// Finds labeled change only when the label is configured.
    #[test]
    fn change_label_requires_opt_in() {
        let keys = keys();
        let payment = SpPaymentBuilder::new()
            .with_inputs(&[(Data::outpoint(0x33, 0), Data::secret_key(0xB1))])
            .with_value(7_000)
            .build(&keys.labeled_address(CHANGE_LABEL));

        assert!(SilentPaymentTxScanner::new(keys)
            .scan_tx(&payment.tx, payment.prevout_lookup())
            .is_empty());

        let found = SilentPaymentTxScanner::new(keys)
            .with_labels([CHANGE_LABEL])
            .scan_tx(&payment.tx, payment.prevout_lookup());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, Some(CHANGE_LABEL));
        assert!(found[0].is_change());
        assert_eq!(found[0].unblinded.value, 7_000);
        assert_eq!(
            found[0]
                .spend_tweak
                .applied_to(&keys.spend_pubkey())
                .unwrap(),
            found[0].output.spend_pubkey
        );
    }

    /// A match at `k` advances `k` even when the output does not unblind, otherwise a sender
    /// could hide later outputs by exhausting the gap limit with unblindable ones.
    #[test]
    fn unblindable_match_does_not_stop_the_scan() {
        let keys = keys();
        let inputs = [(Data::outpoint(0x44, 0), Data::secret_key(0xC1))];
        let sender = SilentPaymentSender::from_inputs(&inputs).unwrap();
        let gap_limit = SilentPaymentScanner::DEFAULT_GAP_LIMIT;

        let mut outputs = Vec::new();
        for k in 0..gap_limit {
            let mut output = sender.derive_output(&keys.address(), k);
            output.blinding_pubkey = Data::material(0xDE, 0xAD).spend_pubkey();
            let (txout, _) =
                SpTxOutBuilder::build(&output, Data::asset(), 1_000, &mut rand::thread_rng())
                    .unwrap();
            outputs.push(txout);
        }

        let payable = sender.derive_output(&keys.address(), gap_limit);
        let (txout, _) =
            SpTxOutBuilder::build(&payable, Data::asset(), 9_000, &mut rand::thread_rng()).unwrap();
        outputs.push(txout);

        let tx = Transaction {
            version: 2,
            lock_time: crate::elements::LockTime::ZERO,
            input: inputs
                .iter()
                .map(|(o, k)| lwk_test_util::ElementsTestData::p2wpkh_input(*o, k))
                .collect(),
            output: outputs,
        };
        let prevouts: Vec<_> = inputs
            .iter()
            .map(|(o, k)| (*o, lwk_test_util::ElementsTestData::p2wpkh(k)))
            .collect();
        let lookup = |o: &OutPoint| prevouts.iter().find(|(p, _)| p == o).map(|(_, s)| s);

        let found = SilentPaymentTxScanner::new(keys).scan_tx(&tx, lookup);

        assert_eq!(
            found.len(),
            1,
            "the unblindable outputs must be dropped, the payable one kept"
        );
        assert_eq!(
            found[0].k, gap_limit,
            "a match must advance k even when it does not unblind"
        );
        assert_eq!(found[0].unblinded.value, 9_000);
    }

    #[test]
    fn tweak_path_agrees_with_prevout_path() {
        let keys = keys();
        let payment = SpPaymentBuilder::new().with_value(12_345).build_for(&keys);
        let scanner = SilentPaymentTxScanner::new(keys);

        let from_prevouts = scanner.scan_tx(&payment.tx, payment.prevout_lookup());

        let extracted = SilentPaymentTxInputs::extract(&payment.tx, payment.prevout_lookup());
        let tweak = PartialTweak::from_observed(&extracted.observed().unwrap());
        let from_tweak = scanner.scan_tx_with_tweak(&payment.tx, &tweak);

        assert_eq!(from_prevouts.len(), 1);
        assert_eq!(from_tweak.len(), 1);
        assert_eq!(from_prevouts[0].outpoint, from_tweak[0].outpoint);
        assert_eq!(from_prevouts[0].spend_tweak, from_tweak[0].spend_tweak);
    }
}
