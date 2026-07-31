//! Scans block ranges for silent-payment outputs.

use crate::elements::{Script, Txid};
use crate::silentpayments::{PartialTweak, SilentPaymentScanMaterial, SilentPaymentScanner};
use std::collections::HashMap;

/// Plans a silent-payment scan over transaction tweaks.
#[derive(Debug, Clone)]
pub struct SilentPaymentSync {
    material: SilentPaymentScanMaterial,
    labels: Vec<u32>,
    candidates_per_tx: u32,
}

impl SilentPaymentSync {
    /// Candidate output indices per transaction.
    pub const DEFAULT_CANDIDATES_PER_TX: u32 = 3;

    /// Scans transactions for the plain address.
    pub fn new(material: SilentPaymentScanMaterial) -> Self {
        SilentPaymentSync {
            material,
            labels: Vec::new(),
            candidates_per_tx: Self::DEFAULT_CANDIDATES_PER_TX,
        }
    }

    /// Also watch these BIP-352 labels (e.g. [`crate::silentpayments::CHANGE_LABEL`]).
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = u32>) -> Self {
        self.labels = labels.into_iter().collect();
        self
    }

    /// Sets the candidate count, capped at [`SilentPaymentScanner::K_MAX`].
    pub fn with_candidates_per_tx(mut self, n: u32) -> Self {
        self.candidates_per_tx = n.min(SilentPaymentScanner::K_MAX);
        self
    }

    /// Maps candidate scripts to their source transactions.
    pub fn candidate_scripts(&self, tweaks: &[(Txid, PartialTweak)]) -> HashMap<Script, Txid> {
        let scanner = self.scanner();
        let mut out = HashMap::new();
        for (txid, tweak) in tweaks {
            for script in scanner.candidate_scripts(tweak, self.candidates_per_tx) {
                out.entry(script).or_insert(*txid);
            }
        }
        out
    }

    /// Returns tweaks whose candidate scripts exist on chain.
    pub fn tweaks_to_scan(
        &self,
        tweaks: &[(Txid, PartialTweak)],
        found_scripts: &[Script],
    ) -> Vec<(Txid, PartialTweak)> {
        let by_script = self.candidate_scripts(tweaks);
        let hit: std::collections::HashSet<Txid> = found_scripts
            .iter()
            .filter_map(|s| by_script.get(s).copied())
            .collect();

        tweaks
            .iter()
            .filter(|(txid, _)| hit.contains(txid))
            .cloned()
            .collect()
    }

    /// Returns a scanner configured with this sync's material and labels.
    pub fn scanner(&self) -> SilentPaymentScanner {
        SilentPaymentScanner::new(self.material).with_labels(self.labels.iter().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::{
        SilentPaymentInputs, SilentPaymentScan, SilentPaymentSender, CHANGE_LABEL,
    };

    fn keys() -> SilentPaymentScanMaterial {
        Data::material(0x11, 0x22)
    }

    fn payer(outpoint: u8, key: u8) -> (PartialTweak, SilentPaymentSender) {
        let inputs =
            SilentPaymentInputs::aggregate(&[(Data::outpoint(outpoint, 0), Data::secret_key(key))])
                .unwrap();
        let tweak = PartialTweak::from_observed(&inputs.observed());
        (tweak, SilentPaymentSender::new(inputs))
    }

    #[test]
    fn candidate_generation() {
        let keys = keys();
        let txid = Data::txid(0x01);
        let (tweak, sender) = payer(0x10, 0x31);

        let plain = sender.derive_output(&keys.address(), 0).script_pubkey();
        let labeled = sender
            .derive_output(&keys.labeled_address(CHANGE_LABEL), 0)
            .script_pubkey();

        let unlabeled_scan = SilentPaymentSync::new(keys).candidate_scripts(&[(txid, tweak)]);
        assert_eq!(
            unlabeled_scan.get(&plain),
            Some(&txid),
            "a real payment's script must be a candidate"
        );
        assert!(
            !unlabeled_scan.contains_key(&labeled),
            "unlabeled scan must not generate labeled candidates"
        );

        let labeled_scan = SilentPaymentSync::new(keys)
            .with_labels([CHANGE_LABEL])
            .candidate_scripts(&[(txid, tweak)]);
        assert!(
            labeled_scan.contains_key(&labeled),
            "configured label must generate its candidate"
        );

        assert_eq!(
            SilentPaymentSync::new(keys)
                .with_candidates_per_tx(u32::MAX)
                .candidates_per_tx,
            SilentPaymentScanner::K_MAX,
            "candidate count must stay bounded by K_MAX"
        );
    }

    #[test]
    fn narrowing_keeps_only_hit_transactions() {
        let keys = keys();
        let (our_tweak, our_sender) = payer(0x10, 0x31);
        let (their_tweak, _) = payer(0x20, 0x32);

        let our_txid = Data::txid(0x01);
        let tweaks = vec![(our_txid, our_tweak), (Data::txid(0x02), their_tweak)];
        let our_script = our_sender.derive_output(&keys.address(), 0).script_pubkey();

        let sync = SilentPaymentSync::new(keys);

        let to_scan = sync.tweaks_to_scan(&tweaks, &[our_script]);
        assert_eq!(to_scan.len(), 1, "only the paying tx should be scanned");
        assert_eq!(to_scan[0].0, our_txid);

        assert!(
            sync.tweaks_to_scan(&tweaks, &[]).is_empty(),
            "no candidate seen on chain means no transaction to fetch"
        );
    }
}
