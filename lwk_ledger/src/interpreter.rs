use core::convert::TryFrom;
use core::fmt::Debug;

use elements_miniscript::elements::bitcoin::{
    consensus::encode::{self, VarInt},
    hashes::{sha256, Hash, HashEngine},
};

use crate::{apdu::ClientCommandCode, merkle::MerkleTree};

/// Interpreter for the client-side commands.
/// This struct keeps has methods to keep track of:
///   - known preimages
///   - known Merkle trees from lists of elements
///
/// Moreover, it containes the state that is relevant for the interpreted client side commands:
///   - a queue of bytes that contains any bytes that could not fit in a response from the
///     GET_PREIMAGE client command (when a preimage is too long to fit in a single message) or the
///     GET_MERKLE_LEAF_PROOF command (which returns a Merkle proof, which might be too long to fit
///     in a single message). The data in the queue is returned in one (or more) successive
///     GET_MORE_ELEMENTS commands from the hardware wallet.
///
/// Finally, it keeps track of the yielded values (that is, the values sent from the hardware
/// wallet with a YIELD client command).
pub struct ClientCommandInterpreter {
    yielded: Vec<Vec<u8>>,
    queue: Vec<Vec<u8>>,
    known_preimages: Vec<([u8; 32], Vec<u8>)>,
    trees: Vec<MerkleTree>,
}

impl ClientCommandInterpreter {
    pub fn new() -> Self {
        Self {
            yielded: Vec::new(),
            queue: Vec::new(),
            known_preimages: Vec::new(),
            trees: Vec::new(),
        }
    }

    /// Adds a preimage to the list of known preimages.
    /// The client must respond with `element` when a GET_PREIMAGE command is sent with
    /// `sha256(element)` in its request.
    #[allow(unused)]
    pub fn add_known_preimage(&mut self, element: Vec<u8>) {
        let mut engine = sha256::Hash::engine();
        engine.input(&element);
        let hash = sha256::Hash::from_engine(engine).to_byte_array();
        self.known_preimages.push((hash, element));
    }

    /// Adds a known Merkleized list.
    /// Builds the Merkle tree of `elements`, and adds it to the Merkle trees known to the client
    /// (mapped by Merkle root `mt_root`).
    /// Moreover, adds all the leafs (after adding the b'\0' prefix) to the list of known preimages.
    /// If `el` is one of `elements`, the client must respond with b'\0' + `el` when a GET_PREIMAGE
    /// client command is sent with `sha256(b'\0' + el)`.
    /// Moreover, the commands GET_MERKLE_LEAF_INDEX and GET_MERKLE_LEAF_PROOF must correctly answer
    /// queries relative to the Merkle whose root is `mt_root`.
    pub fn add_known_list(&mut self, elements: &[impl AsRef<[u8]>]) -> [u8; 32] {
        let mut leaves = Vec::with_capacity(elements.len());
        for element in elements {
            let mut preimage = vec![0x00];
            preimage.extend_from_slice(element.as_ref());
            let mut engine = sha256::Hash::engine();
            engine.input(&preimage);
            let hash = sha256::Hash::from_engine(engine).to_byte_array();
            self.known_preimages.push((hash, preimage));
            leaves.push(hash);
        }
        let tree = MerkleTree::new(leaves);
        let root_hash = *tree.root_hash();
        self.trees.push(tree);
        root_hash
    }

    /// Adds the Merkle trees of keys, and the Merkle tree of values (ordered by key)
    /// of a mapping of bytes to bytes.
    /// Adds the Merkle tree of the list of keys, and the Merkle tree of the list of corresponding
    /// values, with the same semantics as the `add_known_list` applied separately to the two lists.
    #[allow(unused)]
    pub fn add_known_mapping(&mut self, mapping: &[(Vec<u8>, Vec<u8>)]) {
        let mut sorted: Vec<&(Vec<u8>, Vec<u8>)> = mapping.iter().collect();
        sorted.sort_by(|(k1, _), (k2, _)| k1.as_slice().cmp(k2));

        let mut keys = Vec::with_capacity(sorted.len());
        let mut values = Vec::with_capacity(sorted.len());
        for (key, value) in sorted {
            keys.push(key.as_slice());
            values.push(value.as_slice());
        }
        self.add_known_list(&keys);
        self.add_known_list(&values);
    }

    // Interprets the client command requested by the hardware wallet, returns the appropriate
    // response to transmit back and updates interpreter internal states.
    pub fn execute(&mut self, command: Vec<u8>) -> Result<Vec<u8>, InterpreterError> {
        if command.is_empty() {
            return Err(InterpreterError::EmptyInput);
        }
        match ClientCommandCode::try_from(command[0]) {
            Ok(ClientCommandCode::Yield) => {
                self.yielded.push(command[1..].to_vec());
                Ok(Vec::new())
            }
            Ok(ClientCommandCode::GetPreimage) => {
                get_preimage_command(&mut self.queue, &self.known_preimages, &command[1..])
            }
            Ok(ClientCommandCode::GetMerkleLeafProof) => {
                get_merkle_leaf_proof(&mut self.queue, &self.trees, &command[1..])
            }
            Ok(ClientCommandCode::GetMerkleLeafIndex) => {
                get_merkle_leaf_index(&self.trees, &command[1..])
            }
            Ok(ClientCommandCode::GetMoreElements) => get_more_elements(&mut self.queue),
            Err(()) => Err(InterpreterError::UnknownCommand(command[0])),
        }
    }

    /// Consumes the interpreter and returns the yielded results.
    #[allow(unused)]
    pub fn yielded(self) -> Vec<Vec<u8>> {
        self.yielded
    }
}

fn get_preimage_command(
    queue: &mut Vec<Vec<u8>>,
    known_preimages: &[([u8; 32], Vec<u8>)],
    request: &[u8],
) -> Result<Vec<u8>, InterpreterError> {
    if request.len() != 33 || request[0] != b'\0' {
        return Err(InterpreterError::UnsupportedRequest(
            ClientCommandCode::GetPreimage as u8,
        ));
    };

    let (_, preimage) = known_preimages
        .iter()
        .find(|(hash, _)| hash == &request[1..])
        .ok_or(InterpreterError::UnknownHash)?;

    let preimage_len_out = encode::serialize(&VarInt(preimage.len() as u64));

    // We can send at most 255 - len(preimage_len_out) - 1 bytes in a single message;
    //the rest will be stored for GET_MORE_ELEMENTS
    let max_payload_size = 255 - preimage_len_out.len() - 1;

    let payload_size = if preimage.len() > max_payload_size {
        max_payload_size
    } else {
        preimage.len()
    };

    if payload_size < preimage.len() {
        for byte in &preimage[payload_size..] {
            queue.push(vec![*byte]);
        }
    }

    let mut response = preimage_len_out;
    response.extend_from_slice(&(payload_size as u8).to_be_bytes());
    response.extend_from_slice(&preimage[..payload_size]);
    Ok(response)
}

fn get_merkle_leaf_proof(
    queue: &mut Vec<Vec<u8>>,
    trees: &[MerkleTree],
    request: &[u8],
) -> Result<Vec<u8>, InterpreterError> {
    if !queue.is_empty() {
        return Err(InterpreterError::UnexpectedQueue);
    } else if request.len() < 34 {
        return Err(InterpreterError::UnsupportedRequest(
            ClientCommandCode::GetMerkleLeafProof as u8,
        ));
    };

    let root = &request[0..32];
    let (tree_size, read): (VarInt, usize) =
        encode::deserialize_partial(&request[32..]).map_err(|_| {
            InterpreterError::UnsupportedRequest(ClientCommandCode::GetMerkleLeafProof as u8)
        })?;

    // deserialize consumes the entire vector.
    let leaf_index: VarInt = encode::deserialize(&request[32 + read..]).map_err(|_| {
        InterpreterError::UnsupportedRequest(ClientCommandCode::GetMerkleLeafProof as u8)
    })?;

    let tree = trees
        .iter()
        .find(|tree| tree.root_hash() == root)
        .ok_or(InterpreterError::UnknownHash)?;

    if leaf_index >= tree_size || tree_size.0 != tree.size() as u64 {
        return Err(InterpreterError::InvalidIndexOrSize);
    }

    let proof = tree
        .get_leaf_proof(leaf_index.0 as usize)
        .ok_or(InterpreterError::InvalidIndexOrSize)?;

    let len_proof = proof.len();
    let mut first_part_proof = Vec::new();
    let mut n_response_elements = 0;
    for (i, p) in proof.into_iter().enumerate() {
        // how many elements we can fit in 255 - 32 - 1 - 1 = 221 bytes ?
        // response: 6 array of 32 bytes.
        if i < 6 {
            first_part_proof.extend(p);
            n_response_elements += 1;
        } else {
            // Add to the queue any proof elements that do not fit the response
            queue.push(p);
        }
    }

    let mut response = tree.get_leaf(leaf_index.0 as usize).unwrap().to_vec();
    response.extend_from_slice(&(len_proof as u8).to_be_bytes());
    response.extend_from_slice(&(n_response_elements as u8).to_be_bytes());
    response.extend_from_slice(&first_part_proof);
    Ok(response)
}

fn get_merkle_leaf_index(
    trees: &[MerkleTree],
    request: &[u8],
) -> Result<Vec<u8>, InterpreterError> {
    if request.len() < 64 {
        return Err(InterpreterError::UnsupportedRequest(
            ClientCommandCode::GetMerkleLeafIndex as u8,
        ));
    }
    let root = &request[0..32];
    let hash = &request[32..64];

    let tree = trees
        .iter()
        .find(|tree| tree.root_hash() == root)
        .ok_or(InterpreterError::UnknownHash)?;

    let leaf_index = tree
        .get_leaf_index(hash)
        .ok_or(InterpreterError::UnknownHash)?;

    let mut response = 1_u8.to_be_bytes().to_vec();
    response.extend(encode::serialize(&VarInt(leaf_index as u64)));
    Ok(response)
}

fn get_more_elements(queue: &mut Vec<Vec<u8>>) -> Result<Vec<u8>, InterpreterError> {
    if queue.is_empty() {
        return Err(InterpreterError::UnexpectedQueue);
    }

    // The queue must contain only element of the same length.
    let element_length = queue[0].len();
    if queue.iter().any(|e| e.len() != element_length) {
        return Err(InterpreterError::UnexpectedQueue);
    }

    let mut response_elements = Vec::new();
    let mut n_added_elements = 0;
    for element in queue.iter() {
        if response_elements.len() + element_length <= 253 {
            response_elements.extend_from_slice(element);
            n_added_elements += 1;
        }
    }
    *queue = queue[n_added_elements..].to_vec();

    let mut response = (n_added_elements as u8).to_be_bytes().to_vec();
    response.extend((element_length as u8).to_be_bytes());
    response.extend(response_elements);
    Ok(response)
}

/// Returns a serialized Merkleized map commitment, encoded as the concatenation of:
///     - the number of key/value pairs, as a Bitcoin-style varint;
///     - the root of the Merkle tree of the keys
///     - the root of the Merkle tree of the values.
#[allow(unused)]
pub fn get_merkleized_map_commitment(mapping: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let mut sorted: Vec<&(Vec<u8>, Vec<u8>)> = mapping.iter().collect();
    sorted.sort_by(|(k1, _), (k2, _)| k1.as_slice().cmp(k2));

    let mut keys_hashes: Vec<[u8; 32]> = Vec::with_capacity(sorted.len());
    let mut values_hashes: Vec<[u8; 32]> = Vec::with_capacity(sorted.len());
    for (key, value) in &sorted {
        let mut preimage = vec![0x00];
        preimage.extend_from_slice(key);
        let mut engine = sha256::Hash::engine();
        engine.input(&preimage);
        keys_hashes.push(sha256::Hash::from_engine(engine).to_byte_array());

        let mut preimage = vec![0x00];
        preimage.extend_from_slice(value);
        let mut engine = sha256::Hash::engine();
        engine.input(&preimage);
        values_hashes.push(sha256::Hash::from_engine(engine).to_byte_array());
    }

    let mut commitment = encode::serialize(&VarInt(sorted.len() as u64));
    commitment.extend(MerkleTree::new(keys_hashes).root_hash());
    commitment.extend(MerkleTree::new(values_hashes).root_hash());
    commitment
}

#[derive(Debug)]
pub enum InterpreterError {
    EmptyInput,
    UnknownCommand(u8),
    UnsupportedRequest(u8),
    InvalidIndexOrSize,
    UnknownHash,
    UnknownMerkleRoot,
    UnexpectedQueue,
}
