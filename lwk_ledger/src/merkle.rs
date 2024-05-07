use elements_miniscript::elements::bitcoin::hashes::{sha256, Hash, HashEngine};

/// MerkleTree is containing a merkle tree generated from a list of items.
pub struct MerkleTree {
    root: Tree,
    leaves: Vec<[u8; 32]>,
}

impl MerkleTree {
    pub fn new(leaves: Vec<[u8; 32]>) -> Self {
        Self {
            root: Tree::new(&leaves, 0, leaves.len()),
            leaves,
        }
    }

    pub fn size(&self) -> usize {
        self.leaves.len()
    }

    /// Returns the root hash of the Merkle tree.
    pub fn root_hash(&self) -> &[u8; 32] {
        match &self.root {
            Tree::Node { value, .. } => value,
            Tree::Leaf(idx) => &self.leaves[*idx],
        }
    }

    /// Returns the leaf value at index i.
    pub fn get_leaf(&self, i: usize) -> Option<&[u8; 32]> {
        self.leaves.get(i)
    }

    /// Get position of the leaf in the tree.
    pub fn get_leaf_index(&self, val: &[u8]) -> Option<usize> {
        self.leaves.iter().position(|v| v == val)
    }

    // Get Merkle proof of a leaf with the given index.
    pub fn get_leaf_proof(&self, index: usize) -> Option<Vec<Vec<u8>>> {
        if index >= self.leaves.len() {
            // Out of bound
            None
        } else {
            Some(self.root.get_proof(&self.leaves, index))
        }
    }
}

/// Tree is either a Node with children trees or a Leaf with only a given value.
enum Tree {
    Node {
        value: [u8; 32],
        left: Box<Tree>,
        right: Box<Tree>,
        height: usize,
    },
    // index of the leaf in the leaves array
    Leaf(usize),
}

impl Tree {
    fn new(leaves: &[[u8; 32]], start: usize, size: usize) -> Self {
        if size == 1 {
            return Tree::Leaf(start);
        }

        let lchild_size = largest_power_of_2_less_than(size);
        let lchild = Tree::new(leaves, start, lchild_size);
        let rchild = Tree::new(leaves, start + lchild_size, size - lchild_size);

        let mut input = vec![0x01];
        input.extend_from_slice(lchild.value(leaves));
        input.extend_from_slice(rchild.value(leaves));

        let mut engine = sha256::Hash::engine();
        engine.input(input.as_slice());
        let value = sha256::Hash::from_engine(engine).to_byte_array();
        Tree::Node {
            height: lchild.height() + 1,
            left: Box::new(lchild),
            right: Box::new(rchild),
            value,
        }
    }

    fn value<'a>(&'a self, leaves: &'a [[u8; 32]]) -> &'a [u8; 32] {
        match self {
            Self::Node { value, .. } => value,
            Self::Leaf(idx) => &leaves[*idx],
        }
    }

    fn height(&self) -> usize {
        match self {
            Self::Node { height, .. } => *height,
            Self::Leaf(_) => 0,
        }
    }

    /// get the merkle proof of a leaf with the given index in the leaves array.
    fn get_proof(&self, leaves: &[[u8; 32]], index: usize) -> Vec<Vec<u8>> {
        match self {
            Self::Leaf(_) => Vec::new(),
            Self::Node { left, right, .. } => {
                let (mut proof, sibling) = if index < pow2(left.height()) {
                    (left.get_proof(leaves, index), right)
                } else {
                    (right.get_proof(leaves, index - pow2(left.height())), left)
                };
                match **sibling {
                    Self::Node { value, .. } => proof.push(value.to_vec()),
                    Self::Leaf(idx) => proof.push(leaves[idx].to_vec()),
                }
                proof
            }
        }
    }
}

/// Return floor(log_2(n)) for a positive integer `n`.
fn floor_lg(n: usize) -> usize {
    assert!(n > 0);

    let mut r = 0;
    let mut t = 1;
    while 2 * t <= n {
        t *= 2;
        r += 1;
    }
    r
}

fn pow2(n: usize) -> usize {
    let mut p = 1;
    for _i in 0..n {
        p *= 2
    }
    p
}

/// For a positive integer `n`, returns `True` is `n` is a perfect power of 2, `False` otherwise.
fn is_power_of_2(n: usize) -> bool {
    assert!(n >= 1);
    n & (n - 1) == 0
}

/// For an integer `n` which is at least 2, returns the largest exact power of 2 that is strictly less than `n`.
fn largest_power_of_2_less_than(n: usize) -> usize {
    if n == 2 {
        return 1;
    }
    assert!(n > 1);
    if is_power_of_2(n) {
        n / 2
    } else {
        1 << floor_lg(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements_miniscript::elements::bitcoin::hashes::{sha256, Hash, HashEngine};

    #[test]
    fn test_merkle_tree() {
        let leaves = [
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ],
            [
                0xd7, 0xa8, 0xfb, 0xb3, 0x07, 0xd7, 0x80, 0x94, 0x69, 0xca, 0x9a, 0xbc, 0xb0, 0x08,
                0x2e, 0x4f, 0x8d, 0x56, 0x51, 0xe4, 0x6d, 0x3c, 0xdb, 0x76, 0x2d, 0x02, 0xd0, 0xbf,
                0x37, 0xc9, 0xe5, 0x92,
            ],
            [
                0xef, 0x53, 0x7f, 0x25, 0xc8, 0x95, 0xbf, 0xa7, 0x82, 0x52, 0x65, 0x29, 0xa9, 0xb6,
                0x3d, 0x97, 0xaa, 0x63, 0x15, 0x64, 0xd5, 0xd7, 0x89, 0xc2, 0xb7, 0x65, 0x44, 0x8c,
                0x86, 0x35, 0xfb, 0x6c,
            ],
            [
                0x00, 0x53, 0x7f, 0x25, 0xc8, 0x95, 0xbf, 0xa7, 0x82, 0x52, 0x65, 0x29, 0xa9, 0xb6,
                0x3d, 0x00, 0xaa, 0x63, 0x15, 0x64, 0xd5, 0xd7, 0x89, 0xc2, 0xb7, 0x65, 0x44, 0x8c,
                0x86, 0x35, 0xfb, 0x6c,
            ],
        ];

        let tree = MerkleTree::new(leaves[0..3].to_vec());

        assert_eq!(
            tree.get_leaf_proof(0),
            Some(vec![leaves[1].to_vec(), leaves[2].to_vec()])
        );

        assert_eq!(
            tree.get_leaf_proof(1),
            Some(vec![leaves[0].to_vec(), leaves[2].to_vec()])
        );

        let mut input = vec![0x01];
        input.extend_from_slice(&leaves[0]);
        input.extend_from_slice(&leaves[1]);

        let mut engine = sha256::Hash::engine();
        engine.input(input.as_slice());
        let value = sha256::Hash::from_engine(engine).to_byte_array();
        assert_eq!(tree.get_leaf_proof(2), Some(vec![value.to_vec()]));

        let _tree = MerkleTree::new(leaves.to_vec());
    }
}
