use elements_miniscript::elements::{encode::Decodable, hex::FromHex, Block, BlockHeader};

pub fn liquid_block_1() -> Block {
    let raw = include_bytes!(
        "../test_data/afafbbdfc52a45e51a3b634f391f952f6bdfd14ef74b34925954b4e20d0ad639.raw"
    );
    Block::consensus_decode(&raw[..]).unwrap()
}

pub fn liquid_block_header_2_963_520() -> BlockHeader {
    let hex = include_str!("../test_data/block_header_2_963_520.hex");
    let bytes = Vec::<u8>::from_hex(hex).unwrap();
    BlockHeader::consensus_decode(&bytes[..]).unwrap()
}
