//! Silent payment outputs owned by the wallet.

use crate::elements::{OutPoint, TxOut, TxOutSecrets};
use crate::model::ExternalUtxo;
use crate::silentpayments::{LabeledHit, SilentPaymentOutput, SpendTweak, CHANGE_LABEL};

/// A scanned, unblinded silent-payment output and its spend tweak.
#[derive(Debug, Clone)]
pub struct SilentPaymentUtxo {
    /// Where the output sits on chain.
    pub outpoint: OutPoint,

    /// The output as it appears in the transaction, still blinded.
    pub txout: TxOut,

    /// Asset and value, unblinded with `bk_k`.
    pub unblinded: TxOutSecrets,

    /// The output counter `k` this output was found at.
    pub k: u32,

    /// The label the payment was sent to; `Some(CHANGE_LABEL)` is the wallet's change.
    pub label: Option<u32>,

    /// The recomputed spend and blinding key material.
    pub output: SilentPaymentOutput,

    /// `t_k (+ label_tweak_m)`, added to `b_spend` when signing.
    pub spend_tweak: SpendTweak,
}

impl SilentPaymentUtxo {
    /// Weight of a default-sighash key-path Taproot satisfaction.
    pub const MAX_WEIGHT_TO_SATISFY: usize = 66;

    /// Assemble a found output from a scan hit and the located transaction output.
    pub fn new(outpoint: OutPoint, txout: TxOut, unblinded: TxOutSecrets, hit: LabeledHit) -> Self {
        SilentPaymentUtxo {
            outpoint,
            txout,
            unblinded,
            k: hit.k,
            label: hit.label,
            output: hit.output,
            spend_tweak: hit.spend_tweak,
        }
    }

    /// The cache's view of this output.
    pub fn cache_entry(&self) -> crate::silentpayments::SilentPaymentCacheEntry {
        crate::silentpayments::SilentPaymentCacheEntry {
            outpoint: self.outpoint,
            script_pubkey: self.txout.script_pubkey.clone(),
            k: self.k,
            label: self.label,
            spend_tweak: self.spend_tweak,
            blinding_pubkey: self.output.blinding_pubkey,
        }
    }

    /// Whether this output is the wallet's own silent-payment change (label `m = 0`).
    pub fn is_change(&self) -> bool {
        self.label == Some(CHANGE_LABEL)
    }

    /// Returns the descriptor-independent funding view.
    pub fn external_utxo(&self) -> ExternalUtxo {
        ExternalUtxo {
            outpoint: self.outpoint,
            txout: self.txout.clone(),
            tx: None,
            unblinded: self.unblinded,
            max_weight_to_satisfy: Self::MAX_WEIGHT_TO_SATISFY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wrong value here under-pays the fee on every transaction spending a silent
    /// payment, so the constant is pinned against miniscript rather than trusted.
    #[test]
    fn sp_taproot_satisfaction_weight_matches_miniscript() {
        use elements_miniscript::{Descriptor, DescriptorPublicKey};
        use std::str::FromStr;

        let pk = lwk_test_util::ElementsTestData::public_key(0x42);
        let desc = Descriptor::<DescriptorPublicKey>::from_str(&format!("eltr({pk})"))
            .expect("tr descriptor parses");

        assert_eq!(
            desc.max_weight_to_satisfy().expect("weight is computable"),
            SilentPaymentUtxo::MAX_WEIGHT_TO_SATISFY,
            "hard-coded SP taproot satisfaction weight disagrees with miniscript"
        );
    }
}
