//! Verify that a transaction is included in a block, given a merkle proof.

use elements::hashes::{sha256d, HashEngine};
use elements::{TxMerkleNode, Txid};

/// A merkle proof that a transaction is included in a block.
///
/// **Experimental**: this API might change without notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxInclusionProof {
    branch: Vec<TxMerkleNode>,
    pos: u32,
}

impl TxInclusionProof {
    /// Creates a proof from its branch and the position of the transaction in the block.
    ///
    /// **Experimental**: this API might change without notice.
    pub fn new(branch: Vec<TxMerkleNode>, pos: u32) -> Self {
        Self { branch, pos }
    }

    /// The sibling nodes on the path from the transaction to the merkle root.
    pub fn branch(&self) -> &[TxMerkleNode] {
        &self.branch
    }

    /// The position of the transaction in the block.
    pub fn pos(&self) -> u32 {
        self.pos
    }
}

/// Checks that `txid` is committed to by `root`, the merkle root of a block header.
///
/// **Experimental**: this API might change without notice.
pub fn verify_tx_inclusion(txid: Txid, proof: &TxInclusionProof, root: TxMerkleNode) -> bool {
    // Branch depth is not checked (CVE-2017-12842): Elements policy doesn't relay transactions
    // under 65 bytes, see
    // https://github.com/ElementsProject/elements/blob/3c40bb994720616841715448cb9957ada984c556/src/validation.cpp#L963-L965
    if proof.branch.len() > 32 {
        return false;
    }

    // A branch of n elements can only describe a tree of at most 2^n leaves.
    let max_leaves = 1u32
        .checked_shl(proof.branch.len() as u32)
        .unwrap_or(u32::MAX);
    if proof.pos >= max_leaves {
        return false;
    }

    let mut pos = proof.pos;
    let mut current = txid.to_byte_array();
    for node in &proof.branch {
        let sibling = node.to_byte_array();

        // Padded last node of odd-sized level makes position bit irrelevant (CVE-2012-2459), see
        // https://github.com/bitcoin/bitcoin/blob/66531447adb6f647037de563c3cfbf1993cd398e/src/consensus/merkle.cpp#L16-L50
        if !pos.is_multiple_of(2) && sibling == current {
            return false;
        }

        let mut engine = sha256d::Hash::engine();
        if pos.is_multiple_of(2) {
            engine.input(&current);
            engine.input(&sibling);
        } else {
            engine.input(&sibling);
            engine.input(&current);
        }
        current = sha256d::Hash::from_engine(engine).to_byte_array();
        pos /= 2;
    }

    current == root.to_byte_array()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Liquid mainnet block 3899993, 11 transactions.
    const ROOT_11: &str = "c8383d87197525985c3fab9abce4c3652eae6bfa744f2a07debb4e851f02387e";

    /// Liquid mainnet block 4047153, 2 transactions.
    const ROOT_2: &str = "2054eee16702a42832be3643c1d5ada64c08d28df256d76ce08deb4a690b2c58";

    /// Liquid mainnet block 4047150, 1 transaction: the root is the coinbase txid itself.
    const ROOT_1: &str = "0caf7b9bcbe8c7bba91928aa4424af7e43c6fe821c7c7b812a459a1e57b82fc3";

    /// Forged roots: ROOT_1 hashed with a [1; 32] sibling for 32 and 33 levels.
    const ROOT_1_DEPTH_32: &str =
        "2d59997108d6bc3359e583f4f157267c8ac5aaadb601c29e3be5bd8400ffd167";
    const ROOT_1_DEPTH_33: &str =
        "78098e5ac99ec4854847fe8e64ef90f3f8675cfdb3ce012440880add6e97a37a";

    fn txid(s: &str) -> Txid {
        Txid::from_str(s).unwrap()
    }

    fn root(s: &str) -> TxMerkleNode {
        TxMerkleNode::from_str(s).unwrap()
    }

    fn proof(branch: &[&str], pos: u32) -> TxInclusionProof {
        TxInclusionProof::new(branch.iter().map(|s| root(s)).collect(), pos)
    }

    /// The first transaction of block 3899993, at an even position.
    fn first_of_11() -> (Txid, TxInclusionProof) {
        let t = txid("351ea736290891dc9f92d2ef5666fa1146b4d540694b697c22a9755651503ff5");
        let p = proof(
            &[
                "7665ec15e1a35a1489966ebd385a2953ce1c42fe4f615d2c574a3636df7c68df",
                "ef5b3b2326a770ae618dd5d381b599dc88baa8bdb16ab8a52b65aa1ec4294989",
                "663df5fe5180107e81f769e6ce827cdbb4a87815ca53e08cad3f17cf216d504c",
                "f3fe8fccbe9af290832546e099f6afded605acfcb91411d5d0e942c33a7adaa5",
            ],
            0,
        );
        (t, p)
    }

    /// The second transaction of block 3899993, at an odd position.
    fn second_of_11() -> (Txid, TxInclusionProof) {
        let t = txid("7665ec15e1a35a1489966ebd385a2953ce1c42fe4f615d2c574a3636df7c68df");
        let p = proof(
            &[
                "351ea736290891dc9f92d2ef5666fa1146b4d540694b697c22a9755651503ff5",
                "ef5b3b2326a770ae618dd5d381b599dc88baa8bdb16ab8a52b65aa1ec4294989",
                "663df5fe5180107e81f769e6ce827cdbb4a87815ca53e08cad3f17cf216d504c",
                "f3fe8fccbe9af290832546e099f6afded605acfcb91411d5d0e942c33a7adaa5",
            ],
            1,
        );
        (t, p)
    }

    /// The tenth transaction of block 3899993
    fn tenth_of_11() -> (Txid, TxInclusionProof) {
        let t = txid("546367851293b9a681b50ffca7f322e73e6087d976b79d8419a829e012c169d1");
        let p = proof(
            &[
                "f3805f68ba0d483264a7d4f7e6ca7fdf4e6efa0b22c636aa0863a5886b47e206",
                "37fa98a67fa78e5acf2b4c14890d2b7dd334a7a44d6861f2822dd6f48963853a",
                "5c72a8824b045da54fa76970a5c76f0a49ccb935ebacd6d52f3e8091b8c3f3af",
                "1989e959816a27bac46cb2c4f434140085f648308ec6b1491c27c3e93b0ccae5",
            ],
            9,
        );
        (t, p)
    }

    /// The last transaction of block 3899993: an odd number of leaves, so the first step
    /// of the fold pairs it with itself.
    fn last_of_11() -> (Txid, TxInclusionProof) {
        let t = txid("a5fa6f63298b3b8acfb29535dc7f10d1c269196a2132659c0c0874546060bcba");
        let p = proof(
            &[
                "a5fa6f63298b3b8acfb29535dc7f10d1c269196a2132659c0c0874546060bcba",
                "9093787e13712286e9d42a75745633677295fd03b1efbb16aaa84bfab456ee40",
                "5c72a8824b045da54fa76970a5c76f0a49ccb935ebacd6d52f3e8091b8c3f3af",
                "1989e959816a27bac46cb2c4f434140085f648308ec6b1491c27c3e93b0ccae5",
            ],
            10,
        );
        (t, p)
    }

    #[test]
    fn test_verify_tx_inclusion() {
        for (txid, proof) in [first_of_11(), second_of_11(), tenth_of_11(), last_of_11()] {
            assert!(verify_tx_inclusion(txid, &proof, root(ROOT_11)));
        }

        // A two transaction block, so a single element branch.
        let t = txid("ab4f463c4e1a6ddfbcac21e18b263377a16a43a082f619abbc23c61bbae80244");
        let p = proof(
            &["fe9a2af204ea4cb6635d20153be32bad2ea84306f594653220a3d48ea8468232"],
            1,
        );
        assert!(verify_tx_inclusion(t, &p, root(ROOT_2)));

        // A single transaction block, so an empty branch: the txid is the root.
        let t = txid(ROOT_1);
        assert!(verify_tx_inclusion(t, &proof(&[], 0), root(ROOT_1)));

        // 32 levels is the deepest tree a u32 position can index.
        let p = TxInclusionProof::new(vec![TxMerkleNode::from_byte_array([1; 32]); 32], 0);
        assert!(verify_tx_inclusion(t, &p, root(ROOT_1_DEPTH_32)));
    }

    #[test]
    fn test_verify_tx_inclusion_fails() {
        // txs from ROOT_11's block
        let (tx_1, proof_1) = first_of_11();

        // Another transaction of the same block, with the wrong branch.
        let (tx_2, _) = second_of_11();
        assert!(!verify_tx_inclusion(tx_2, &proof_1, root(ROOT_11)));

        // The right proof against the root of another block.
        assert!(!verify_tx_inclusion(tx_1, &proof_1, root(ROOT_2)));

        // The right branch at the wrong position.
        let wrong_pos = TxInclusionProof::new(proof_1.branch.clone(), 1);
        assert!(!verify_tx_inclusion(tx_1, &wrong_pos, root(ROOT_11)));

        // Position out of the number of leaves.
        let (tx_last, proof_last) = last_of_11();
        let leaves = 1u32 << proof_last.branch.len();
        assert_eq!(leaves, 16);
        let past_end = TxInclusionProof::new(proof_last.branch.clone(), proof_last.pos + leaves);
        assert!(!verify_tx_inclusion(tx_last, &past_end, root(ROOT_11)));

        // Fake positions, flipping the bits of the levels where the sibling is a copy.
        for pos in [11, 14, 15] {
            let fake = TxInclusionProof::new(proof_last.branch.clone(), pos);
            assert!(!verify_tx_inclusion(tx_last, &fake, root(ROOT_11)));
        }

        // A truncated branch.
        let truncated = TxInclusionProof::new(proof_1.branch[..3].to_vec(), proof_1.pos);
        assert!(!verify_tx_inclusion(tx_1, &truncated, root(ROOT_11)));

        // A branch with the siblings swapped.
        let mut swapped = proof_1.branch.clone();
        swapped.swap(0, 1);
        assert!(!verify_tx_inclusion(
            tx_1,
            &TxInclusionProof::new(swapped, proof_1.pos),
            root(ROOT_11)
        ));

        // branch len out of bounds
        let t = txid(ROOT_1);
        let p = TxInclusionProof::new(vec![TxMerkleNode::from_byte_array([1; 32]); 33], 0);
        assert!(!verify_tx_inclusion(t, &p, root(ROOT_1_DEPTH_33)));
    }
}
