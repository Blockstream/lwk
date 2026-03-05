//! Deterministic bounded Branch-and-Bound helper used for auxiliary input selection.
//!
//! Search semantics:
//! - exact subset-sum search under a hard node cap
//! - deterministic tie-break for equivalent exact subsets
//! - deterministic fallbacks when exact subset is unavailable or search is capped
//!
//! Tie-break for exact matches:
//! 1. fewer selected inputs
//! 2. lexicographically smaller `(txid_lex, vout)` set

use crate::error::WalletAbiError;

/// Auxiliary `BnB` candidate projection used for deterministic subset search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BnbCandidate {
    pub amount_sat: u64,
    pub txid_lex: String,
    pub vout: u32,
}

/// Upper bound on DFS nodes visited by `BnB` before deterministic fallback is used.
const MAX_BNB_NODES: usize = 100_000;

struct BnbSearch<'a> {
    target_sat: u64,
    candidates: &'a [BnbCandidate],
    suffix_sum_sat: &'a [u64],
    max_nodes: usize,
    nodes_visited: usize,
    node_limit_hit: bool,
    current: Vec<usize>,
    best: Option<Vec<usize>>,
}

impl<'a> BnbSearch<'a> {
    fn new(
        target_sat: u64,
        candidates: &'a [BnbCandidate],
        suffix_sum_sat: &'a [u64],
        max_nodes: usize,
    ) -> Self {
        Self {
            target_sat,
            candidates,
            suffix_sum_sat,
            max_nodes,
            nodes_visited: 0,
            node_limit_hit: false,
            current: Vec::new(),
            best: None,
        }
    }

    fn mark_node_visit(&mut self) -> bool {
        self.nodes_visited = self.nodes_visited.saturating_add(1);
        self.nodes_visited > self.max_nodes
    }

    fn record_exact_if_better(&mut self) {
        if self.is_better_exact_subset() {
            self.best = Some(self.current.clone());
        }
    }

    fn can_reach_target(&self, index: usize, sum_sat: u64) -> Result<bool, WalletAbiError> {
        let max_possible = sum_sat
            .checked_add(self.suffix_sum_sat[index])
            .ok_or_else(|| overflow_error("evaluating BnB pruning bounds"))?;

        Ok(max_possible >= self.target_sat)
    }

    fn search(&mut self, index: usize, sum_sat: u64) -> Result<(), WalletAbiError> {
        if self.node_limit_hit {
            return Ok(());
        }
        if self.mark_node_visit() {
            self.node_limit_hit = true;
            return Ok(());
        }

        if sum_sat == self.target_sat {
            self.record_exact_if_better();
            return Ok(());
        }
        if index >= self.candidates.len() || sum_sat > self.target_sat {
            return Ok(());
        }
        if !self.can_reach_target(index, sum_sat)? {
            return Ok(());
        }

        let included_sum = sum_sat
            .checked_add(self.candidates[index].amount_sat)
            .ok_or_else(|| overflow_error("evaluating BnB include branch"))?;
        if included_sum <= self.target_sat {
            self.current.push(index);
            self.search(index + 1, included_sum)?;
            self.current.pop();
        }

        self.search(index + 1, sum_sat)
    }

    /// Compare two exact-match subsets by deterministic tie-break rules.
    ///
    /// Preference:
    /// 1. fewer selected inputs
    /// 2. lexicographically smaller outpoint key
    fn is_better_exact_subset(&self) -> bool {
        let proposed = &self.current;

        let Some(current_best) = self.best.as_deref() else {
            return true;
        };

        if proposed.len() < current_best.len() {
            return true;
        }
        if proposed.len() > current_best.len() {
            return false;
        }

        self.subset_lexicographic_key(proposed) < self.subset_lexicographic_key(current_best)
    }

    /// Build a comparable outpoint-key for one candidate subset.
    ///
    /// The key is sorted so subset order itself does not affect comparisons.
    fn subset_lexicographic_key<'b>(&'b self, indices: &[usize]) -> Vec<(&'b str, u32)> {
        let mut key = indices
            .iter()
            .map(|index| {
                let candidate = &self.candidates[*index];
                (candidate.txid_lex.as_str(), candidate.vout)
            })
            .collect::<Vec<_>>();
        key.sort();
        key
    }
}

fn build_bnb_suffix_sums(candidates: &[BnbCandidate]) -> Result<Vec<u64>, WalletAbiError> {
    let mut suffix_sum_sat = vec![0u64; candidates.len() + 1];
    for index in (0..candidates.len()).rev() {
        suffix_sum_sat[index] = suffix_sum_sat[index + 1]
            .checked_add(candidates[index].amount_sat)
            .ok_or_else(|| overflow_error("computing BnB suffix sums"))?;
    }

    Ok(suffix_sum_sat)
}

/// Bounded depth-first Branch-and-Bound search for an exact subset sum.
///
/// Returns:
/// - selected indices when an exact match is found
///
/// Pruning:
/// - stop include branch when `sum > target`
/// - stop branch when `sum + remaining < target`
pub fn bnb_exact_subset_indices(
    candidates: &[BnbCandidate],
    target_sat: u64,
) -> Result<Option<Vec<usize>>, WalletAbiError> {
    if target_sat == 0 {
        return Ok(Some(Vec::new()));
    }
    if candidates.is_empty() {
        return Ok(None);
    }

    let suffix_sum_sat = build_bnb_suffix_sums(candidates)?;
    let mut search = BnbSearch::new(target_sat, candidates, &suffix_sum_sat, MAX_BNB_NODES);
    search.search(0, 0)?;

    Ok(if search.node_limit_hit {
        None
    } else {
        search.best
    })
}

/// Deterministic fallback A: select one largest UTXO whose amount is `>= target`.
///
/// Candidates are expected to be sorted by amount desc, then txid asc, then vout asc.
pub fn select_single_largest_above_target(
    candidates: &[BnbCandidate],
    target_sat: u64,
) -> Option<Vec<usize>> {
    candidates
        .iter()
        .position(|candidate| candidate.amount_sat >= target_sat)
        .map(|index| vec![index])
}

/// Deterministic fallback B: accumulate largest-first until the target is reached.
///
/// Candidates are expected to be sorted by amount desc, then txid asc, then vout asc.
pub fn select_largest_first_accumulation(
    candidates: &[BnbCandidate],
    target_sat: u64,
) -> Result<Option<Vec<usize>>, WalletAbiError> {
    let mut selected_indices = Vec::new();
    let mut sum_sat = 0u64;

    for (index, candidate) in candidates.iter().enumerate() {
        selected_indices.push(index);
        sum_sat = sum_sat
            .checked_add(candidate.amount_sat)
            .ok_or_else(|| overflow_error("running fallback accumulation"))?;
        if sum_sat >= target_sat {
            return Ok(Some(selected_indices));
        }
    }

    Ok(None)
}

/// Sum selected candidate amounts with overflow checks.
pub fn sum_selected_amount(
    candidates: &[BnbCandidate],
    selected_indices: &[usize],
) -> Result<u64, WalletAbiError> {
    selected_indices.iter().try_fold(0u64, |sum, index| {
        sum.checked_add(candidates[*index].amount_sat)
            .ok_or_else(|| overflow_error("summing selected auxiliary inputs"))
    })
}

fn overflow_error(context: &str) -> WalletAbiError {
    WalletAbiError::InvalidRequest(format!("asset amount overflow while {context}"))
}
