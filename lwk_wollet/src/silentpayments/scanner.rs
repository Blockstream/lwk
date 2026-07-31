//! Gap-limited silent-payment scanning.

use crate::secp256k1::PublicKey;
use crate::silentpayments::{
    PartialTweak, SharedSecret, SilentPaymentOutput, SilentPaymentReceiver, SilentPaymentScan,
    SilentPaymentScanMaterial, SpendTweak,
};
use crate::util::EC;

/// A silent-payment output found by a scan.
#[derive(Debug, Clone)]
pub struct LabeledHit {
    /// Output counter `k`.
    pub k: u32,
    /// The label `m` the output was sent to, if any. `Some(0)` is change.
    pub label: Option<u32>,
    /// The recomputed SP output (spend pubkey + blinding keys).
    pub output: SilentPaymentOutput,
    /// Tweak needed to spend this output.
    pub spend_tweak: SpendTweak,
}

/// Scans transactions for silent-payment outputs.
#[derive(Debug, Clone)]
pub struct SilentPaymentScanner {
    receiver: SilentPaymentReceiver,
    labels: Vec<u32>,
    gap_limit: u32,
}

impl SilentPaymentScanner {
    /// Default consecutive-miss limit.
    pub const DEFAULT_GAP_LIMIT: u32 = 3;

    /// Exclusive BIP-352 output-count limit.
    pub const K_MAX: u32 = 2323;

    /// A scanner for `material` that looks only for plain (unlabeled) outputs.
    pub fn new(material: SilentPaymentScanMaterial) -> Self {
        SilentPaymentScanner {
            receiver: SilentPaymentReceiver::new(material),
            labels: Vec::new(),
            gap_limit: Self::DEFAULT_GAP_LIMIT,
        }
    }

    /// Also detect outputs sent to any of `labels` (e.g.
    /// [`crate::silentpayments::CHANGE_LABEL`]).
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = u32>) -> Self {
        self.labels = labels.into_iter().collect();
        self
    }

    /// Stop scanning after `gap_limit` consecutive misses.
    pub fn with_gap_limit(mut self, gap_limit: u32) -> Self {
        self.gap_limit = gap_limit;
        self
    }

    /// Scans a transaction's partial tweak for owned outputs.
    pub fn scan(
        &self,
        partial_tweak: &PartialTweak,
        output_scripts: &[crate::elements::Script],
    ) -> Vec<LabeledHit> {
        let material = self.receiver.material();
        let shared_secret =
            SharedSecret::from_partial_tweak(&material.scan_seckey(), partial_tweak.as_pubkey());
        let labeled_bases = self.labeled_bases();

        let mut found = Vec::new();
        let mut k = 0u32;
        let mut misses = 0u32;
        while misses < self.gap_limit && k < Self::K_MAX {
            let t_k = shared_secret.spend_tweak(k);
            let mut hit = false;

            let (plain, spend_tweak) = self.receiver.derive_from_shared_secret(&shared_secret, k);
            if output_scripts.iter().any(|s| *s == plain.script_pubkey()) {
                found.push(LabeledHit {
                    k,
                    label: None,
                    output: plain,
                    spend_tweak,
                });
                hit = true;
            }

            for (m, base) in &labeled_bases {
                let spend_pubkey = base.add_exp_tweak(&EC, &t_k).expect("labeled P_k");
                let output = SilentPaymentOutput {
                    spend_pubkey,
                    blinding_pubkey: plain.blinding_pubkey,
                    blinding_seckey: plain.blinding_seckey,
                };
                if output_scripts.iter().any(|s| *s == output.script_pubkey()) {
                    let Some(spend_tweak) =
                        SpendTweak::from_scalar(t_k).add_label_tweak(&material.label_tweak(*m))
                    else {
                        continue;
                    };
                    found.push(LabeledHit {
                        k,
                        label: Some(*m),
                        output,
                        spend_tweak,
                    });
                    hit = true;
                }
            }

            if hit {
                misses = 0;
            } else {
                misses += 1;
            }
            k += 1;
        }
        found
    }

    /// Derives candidate output scripts for `0..count`.
    pub fn candidate_scripts(
        &self,
        partial_tweak: &PartialTweak,
        count: u32,
    ) -> Vec<crate::elements::Script> {
        let material = self.receiver.material();
        let shared_secret =
            SharedSecret::from_partial_tweak(&material.scan_seckey(), partial_tweak.as_pubkey());
        let labeled_bases = self.labeled_bases();

        let mut scripts = Vec::new();
        for k in 0..count.min(Self::K_MAX) {
            let t_k = shared_secret.spend_tweak(k);
            let (plain, _) = self.receiver.derive_from_shared_secret(&shared_secret, k);
            scripts.push(plain.script_pubkey());

            for (_, base) in &labeled_bases {
                let spend_pubkey = base.add_exp_tweak(&EC, &t_k).expect("labeled P_k");
                let output = SilentPaymentOutput {
                    spend_pubkey,
                    blinding_pubkey: plain.blinding_pubkey,
                    blinding_seckey: plain.blinding_seckey,
                };
                scripts.push(output.script_pubkey());
            }
        }
        scripts
    }

    /// Computes labeled spend bases.
    fn labeled_bases(&self) -> Vec<(u32, PublicKey)> {
        self.labels
            .iter()
            .map(|&m| (m, self.receiver.labeled_spend_base(m)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::{SilentPaymentSender, SilentPaymentTweakClient, CHANGE_LABEL};

    /// Verifies a scan hit using public spend material.
    fn assert_hit_verifies(hit: &LabeledHit, m: &SilentPaymentScanMaterial) {
        assert_eq!(
            hit.spend_tweak.applied_to(&m.spend_pubkey()).unwrap(),
            hit.output.spend_pubkey,
            "tweak must reproduce the output spend key at k={}",
            hit.k
        );
    }

    #[test]
    fn scan_finds_every_output_it_should_and_no_others() {
        struct MockServer {
            tweaks: Vec<PartialTweak>,
        }
        impl SilentPaymentTweakClient for MockServer {
            type Error = ();
            fn tweaks(&self, _height: u32) -> Result<Vec<PartialTweak>, ()> {
                Ok(self.tweaks.clone())
            }
        }

        let keys = Data::material(0x11, 0x22);
        let other_label = 7u32;

        let change_addr = keys.labeled_address(CHANGE_LABEL);
        assert_ne!(change_addr.spend, keys.address().spend);
        assert_eq!(change_addr.scan, keys.address().scan);

        let inputs = [
            (Data::outpoint(0x10, 0), Data::secret_key(0x31)),
            (Data::outpoint(0x20, 1), Data::secret_key(0x32)),
        ];
        let sender = SilentPaymentSender::from_inputs(&inputs).unwrap();
        let agg = sender.inputs();

        let plain0 = sender.derive_output(&keys.address(), 0);
        let plain1 = sender.derive_output(&keys.address(), 1);
        let change = sender.derive_output(&change_addr, 0);
        let labeled = sender.derive_output(&keys.labeled_address(other_label), 0);
        let noise = sender.derive_output(&Data::material(0xEE, 0xEF).address(), 0);

        let output_scripts = vec![
            noise.script_pubkey(),
            plain0.script_pubkey(),
            plain1.script_pubkey(),
            change.script_pubkey(),
            labeled.script_pubkey(),
            crate::elements::Script::new(), // fee
        ];

        let server = MockServer {
            tweaks: vec![PartialTweak::new(&agg.a_pubkey, &agg.input_hash)],
        };
        let published = server.tweaks(1).unwrap();

        let plain_scanner = SilentPaymentScanner::new(keys);
        let mut hits = Vec::new();
        for t in &published {
            hits.extend(plain_scanner.scan(t, &output_scripts));
        }
        assert_eq!(hits.len(), 2, "should find both plain SP outputs");
        assert_eq!(hits.iter().map(|h| h.k).collect::<Vec<_>>(), vec![0, 1]);
        for hit in &hits {
            assert_hit_verifies(hit, &keys);
            assert_eq!(hit.label, None, "these are unlabeled outputs");
        }

        let labeled_hits = SilentPaymentScanner::new(keys)
            .with_labels([CHANGE_LABEL, other_label])
            .scan(&published[0], &output_scripts);
        let found_labels: Vec<_> = labeled_hits.iter().filter_map(|h| h.label).collect();
        assert_eq!(
            found_labels.len(),
            2,
            "both labeled outputs should be found"
        );
        assert!(
            found_labels.contains(&CHANGE_LABEL),
            "change label must be recognized"
        );
        assert!(found_labels.contains(&other_label));
        for hit in &labeled_hits {
            assert_hit_verifies(hit, &keys);
        }

        assert!(SilentPaymentScanner::new(Data::material(0x77, 0x88))
            .with_labels([CHANGE_LABEL, other_label])
            .scan(&published[0], &output_scripts)
            .is_empty());
    }

    #[test]
    fn k_max_is_enforced_for_sender_and_scanner() {
        let keys = Data::material(0x11, 0x22);
        let sender =
            SilentPaymentSender::from_inputs(&[(Data::outpoint(0x10, 0), Data::secret_key(0x31))])
                .unwrap();

        assert!(sender
            .try_derive_output(&keys.address(), SilentPaymentScanner::K_MAX - 1)
            .is_some());
        assert!(
            sender
                .try_derive_output(&keys.address(), SilentPaymentScanner::K_MAX)
                .is_none(),
            "sender must refuse to build an output the receiver may not scan for"
        );

        let beyond = sender
            .shared_secret(&keys.address())
            .derive_output(&keys.address().spend, SilentPaymentScanner::K_MAX);
        let t = PartialTweak::from_observed(&sender.inputs().observed());
        let hits = SilentPaymentScanner::new(keys)
            .with_gap_limit(u32::MAX)
            .scan(&t, &[beyond.script_pubkey()]);
        assert!(
            hits.is_empty(),
            "scanner must not find an output past K_max (and must terminate)"
        );
    }
}
