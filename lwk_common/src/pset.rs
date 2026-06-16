use elements::{
    hashes::Hash,
    pset::{raw::ProprietaryKey, PartiallySignedTransaction},
    BlockHash,
};

use crate::Network;

const PSBT_ELEMENTS_GLOBAL_GENESIS_HASH: u8 = 0x02;

// TODO: upstream to rust elements
/// Extract the genesis block hash from the PSET global proprietary fields as defined in
/// [ELIP-101](https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki).
///
/// Returns [`BlockHash::all_zeros`] if the field is absent or malformed.
pub fn get_genesis_hash(pset: &PartiallySignedTransaction) -> BlockHash {
    let key = ProprietaryKey::from_pset_pair(PSBT_ELEMENTS_GLOBAL_GENESIS_HASH, vec![]);
    pset.global
        .proprietary
        .get(&key)
        .and_then(|v| v.as_slice().try_into().ok())
        .map(BlockHash::from_byte_array)
        .unwrap_or(BlockHash::all_zeros())
}

// TODO: upstream to rust elements
// TODO: tested with Jade 1.0.37 but does not work. Safe to merge because subtype is unique.
/// Add genesis block hash as defined in [ELIP-101](https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki)
pub fn set_genesis_hash(pset: &mut PartiallySignedTransaction, network: &Network) {
    let genesis_block_hash = network.genesis_hash().to_byte_array().to_vec();

    pset.global.proprietary.insert(
        ProprietaryKey::from_pset_pair(PSBT_ELEMENTS_GLOBAL_GENESIS_HASH, vec![]),
        genesis_block_hash,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements::encode::{deserialize, serialize};

    #[test]
    fn test_genesis_hash_serde_roundtrip() {
        let network = Network::Liquid;
        let mut pset = PartiallySignedTransaction::new_v2();
        set_genesis_hash(&mut pset, &network);

        let serialized = serialize(&pset);
        let deserialized: PartiallySignedTransaction = deserialize(&serialized).unwrap();

        assert_eq!(get_genesis_hash(&deserialized), network.genesis_hash());
    }
}
