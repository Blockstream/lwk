use elements_miniscript::elements::hex::FromHex;

pub fn update_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
}

pub fn update_test_vector_v1_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v1.hex")).unwrap()
}

pub fn update_test_vector_v4_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v4.hex").trim()).unwrap()
}

pub fn update_test_vector_2_bytes() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector.bin").to_vec()
}

/// An update (serialized v1) with 63 transactions on liquid testnet wallet defined by [`crate::wollet_descriptor_many_transactions`]
pub fn update_test_vector_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_many_txs.bin").to_vec()
}

/// An update (serialized v2) after [`update_test_vector_many_transactions`]
pub fn update_v2_test_vector_after_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_v2_after_many_txs.bin").to_vec()
}

/// First of 3 consecutive updates for merge testing
/// Contains initial wallet sync (tip only)
pub fn update_merge_test_1() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_1.bin").to_vec()
}

/// Second of 3 consecutive updates for merge testing
/// Contains wallet funding transaction
pub fn update_merge_test_2() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_2.bin").to_vec()
}

/// Third of 3 consecutive updates for merge testing
/// Contains spending transaction
pub fn update_merge_test_3() -> Vec<u8> {
    include_bytes!("../test_data/merge_updates/update_merge_3.bin").to_vec()
}

/// Descriptor used for the merge test updates
pub fn update_merge_test_descriptor() -> String {
    include_str!("../test_data/merge_updates/descriptor.txt").to_string()
}

pub fn update_test_vector_encrypted_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!(
        "../test_data/update_test_vector_encrypted.hex"
    ))
    .unwrap()
}

pub fn update_test_vector_encrypted_base64() -> String {
    include_str!("../test_data/update_test_vector/update.base64").to_string()
}

pub fn update_test_vector_encrypted_bytes2() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector/000000000000").to_vec()
}
