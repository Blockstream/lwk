//! Computes silent-payment tweaks for a block.

use crate::elements::{Block, OutPoint, Script, Transaction, Txid};
use crate::silentpayments::{PartialTweak, SilentPaymentTxInputs};
use std::collections::HashMap;

/// Computes silent-payment tweaks for a block view.
pub struct BlockTweaks<'a> {
    block: &'a Block,
}

impl<'a> BlockTweaks<'a> {
    /// A tweak extractor over `block`.
    pub fn new(block: &'a Block) -> Self {
        BlockTweaks { block }
    }

    /// Returns the external prevouts needed to classify block inputs.
    pub fn required_prevouts(&self) -> Vec<OutPoint> {
        let local: HashMap<Txid, &Transaction> =
            self.block.txdata.iter().map(|tx| (tx.txid(), tx)).collect();

        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for tx in &self.block.txdata {
            if tx.is_coinbase() {
                continue;
            }
            for input in &tx.input {
                if input.is_pegin() {
                    continue;
                }
                let op = input.previous_output;
                if local.contains_key(&op.txid) {
                    continue;
                }
                if seen.insert(op) {
                    out.push(op);
                }
            }
        }
        out
    }

    /// Computes partial tweaks for transactions with eligible inputs.
    pub fn compute(&self, prevouts: &HashMap<OutPoint, Script>) -> Vec<(Txid, PartialTweak)> {
        let local: HashMap<Txid, &Transaction> =
            self.block.txdata.iter().map(|tx| (tx.txid(), tx)).collect();

        let mut out = Vec::new();
        for tx in &self.block.txdata {
            if tx.is_coinbase() {
                continue;
            }

            let lookup = |op: &OutPoint| -> Option<&Script> {
                if let Some(script) = prevouts.get(op) {
                    return Some(script);
                }
                local
                    .get(&op.txid)
                    .and_then(|prev| prev.output.get(op.vout as usize))
                    .map(|txout| &txout.script_pubkey)
            };

            let inputs = SilentPaymentTxInputs::extract(tx, lookup);
            if !inputs.is_eligible() {
                continue;
            }
            let Ok(observed) = inputs.observed() else {
                continue;
            };
            out.push((tx.txid(), PartialTweak::from_observed(&observed)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{
        confidential::{Asset, Value},
        TxOut,
    };
    use crate::util::EC;
    use lwk_test_util::ElementsTestData as Data;

    /// Build a P2WPKH-spending transaction so its input is SP-eligible.
    fn spending_tx(prev: OutPoint, secret: u8) -> Transaction {
        let input = Data::p2wpkh_input(prev, &Data::secret_key(secret));
        Transaction {
            version: 2,
            lock_time: crate::elements::LockTime::ZERO,
            input: vec![input],
            output: vec![TxOut {
                asset: Asset::Null,
                value: Value::Explicit(1000),
                nonce: crate::elements::confidential::Nonce::Null,
                script_pubkey: Script::new(),
                witness: Default::default(),
            }],
        }
    }

    fn block_with(txs: Vec<Transaction>) -> Block {
        Block {
            // Reuse the crate's existing test header rather than a bespoke one: the
            // header is irrelevant here, only txdata is read.
            header: crate::update::default_blockheader(),
            txdata: txs,
        }
    }

    /// Every prevout this block genuinely needs from the backend, and nothing else:
    /// same-block outputs are already in hand, peg-in prevouts are Bitcoin txids a
    /// Liquid backend cannot serve, the coinbase spends nothing, and a prevout wanted
    /// twice is still one request.
    #[test]
    fn required_prevouts_asks_only_for_what_it_cannot_derive() {
        let external = Data::outpoint(0x11, 0);
        let funding = spending_tx(Data::outpoint(0x99, 0), 0x41);
        let spends_local = spending_tx(OutPoint::new(funding.txid(), 0), 0x42);
        let spends_external = spending_tx(external, 0x43);
        let also_spends_external = spending_tx(external, 0x44);

        // A peg-in carries a Bitcoin outpoint; the coinbase carries a null one.
        let bitcoin_op = Data::outpoint(0xbc, 0);
        let mut pegin = spending_tx(bitcoin_op, 0x45);
        pegin.input[0].is_pegin = true;
        let mut coinbase = spending_tx(OutPoint::null(), 0x46);
        coinbase.input[0].previous_output = OutPoint::null();

        let block = block_with(vec![
            funding.clone(),
            spends_local,
            spends_external,
            also_spends_external,
            pegin,
            coinbase,
        ]);
        let required = BlockTweaks::new(&block).required_prevouts();

        assert!(required.contains(&external), "external prevout is needed");
        assert!(
            !required.iter().any(|op| op.txid == funding.txid()),
            "same-block prevout must not be fetched"
        );
        assert!(
            !required.contains(&bitcoin_op),
            "a peg-in prevout is a Bitcoin txid and must never be fetched from a Liquid backend"
        );
        assert!(
            !required.contains(&OutPoint::null()),
            "the coinbase spends nothing"
        );
        assert_eq!(
            required.iter().filter(|op| **op == external).count(),
            1,
            "a prevout wanted twice is still one request"
        );
    }

    /// Block-derived tweaks match direct input derivation.
    #[test]
    fn computed_tweak_matches_direct_derivation() {
        let prev = Data::outpoint(0x11, 0);
        let tx = spending_tx(prev, 0x41);
        let block = block_with(vec![tx.clone()]);

        let mut prevouts = HashMap::new();
        prevouts.insert(prev, Data::p2wpkh(&Data::secret_key(0x41)));

        let tweaks = BlockTweaks::new(&block).compute(&prevouts);
        assert_eq!(tweaks.len(), 1);
        assert_eq!(tweaks[0].0, tx.txid());

        let direct = PartialTweak::from_inputs(&[(prev, Data::secret_key(0x41).public_key(&EC))])
            .expect("direct tweak");
        assert_eq!(tweaks[0].1, direct, "block tweak must match direct tweak");

        // Without the prevout script there is no eligible input, so the same
        // transaction must yield no tweak rather than a wrong one.
        assert!(
            BlockTweaks::new(&block).compute(&HashMap::new()).is_empty(),
            "unknown prevout must not produce a tweak"
        );
    }
}
